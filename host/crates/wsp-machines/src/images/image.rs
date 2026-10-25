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
use wsp_core::types::wiring::{GoldenRecipe, PlaceBackends};
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
        todo!()
    }

    /// image.ts:37-41 (I1): restored sizes by snapshot id.
    pub async fn snapshot_sizes(&self, at: Arc<dyn MachineBackend>, ids: &[String]) -> IndexMap<String, f64> {
        todo!()
    }

    /// image.ts:44-67 (I2).
    pub async fn copies_of(&self, name: &str) -> Result<Vec<SealedImageCopy>, HostError> {
        todo!()
    }

    /// image.ts:69 (I3).
    pub fn conflict(&self, message: &str) -> HostError {
        todo!()
    }

    /// image.ts:72-79 (I4).
    pub async fn get(&self, name: Option<&str>) -> Result<SealedImageView, HostError> {
        todo!()
    }

    /// image.ts:81-90 (I5): the record and the vault's bytes.
    pub async fn vault(&self, name: Option<&str>) -> Result<(SealedImage, Vec<u8>), HostError> {
        todo!()
    }

    /// image.ts:92-130 (I6 to I8).
    pub async fn build(self: &Rc<Self>, o: ImageBuildAsk) -> ImageResult<SealedImageBuilt> {
        todo!()
    }

    /// image.ts:132-158 (I9). Never fails.
    pub async fn keep_current(self: &Rc<Self>, place: &str, name: Option<&str>) {
        todo!()
    }

    /// image.ts:163-206 (I10, I11).
    pub async fn build_copy(self: &Rc<Self>, o: CopyBuild) -> ImageResult<SealedImageBuilt> {
        todo!()
    }

    /// image.ts:210-216: the image whose head at its own place is this snapshot.
    pub async fn image_head_named(&self, snapshot_id: &str) -> Result<Option<String>, HostError> {
        todo!()
    }

    /// image.ts:220-227 (I13): the record a version is a copy of.
    pub async fn record_of_version(&self, records: &[SealedImage], version: Option<&wsp_wire::views::golden_image::GoldenVersion>) -> Result<Option<SealedImage>, HostError> {
        todo!()
    }

    /// image.ts:232-240 (I13).
    pub async fn refuse_held_sign_ins(&self, snapshot: &str, place: &str) -> Result<(), HostError> {
        todo!()
    }

    /// image.ts:247-252 (I12, I13).
    pub async fn copy_for_fork(self: &Rc<Self>, golden: &str, place_id: Option<&str>, announce: Option<Announce>) -> ImageResult<String> {
        todo!()
    }

    /// image.ts:254-270 (I12).
    pub async fn snapshot_for_fork(self: &Rc<Self>, golden: &str, place: &str, announce: Option<Announce>) -> ImageResult<String> {
        todo!()
    }
}

