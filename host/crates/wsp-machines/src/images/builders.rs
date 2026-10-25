// SPDX-License-Identifier: AGPL-3.0-only
//! `images/builders.ts` at 65f6b89c7, in two units: `places` (where an image is built and how a builder is shown,
//! spec `builders_places.md`) and `life` (the hold, grace, stages, create, settle and seal, spec `builders_life.md`).
pub mod life;
pub mod places;

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;

use async_trait::async_trait;
use futures::future::LocalBoxFuture;
use indexmap::IndexMap;

use wsp_core::clock::Cancel;
use wsp_core::types::internal::StageFrame;
use wsp_core::types::pending::{Clock, Store};
use wsp_core::types::wiring::{GoldenRecipe, PlaceBackends};
use wsp_engine::machine::{Machine, MachineBackend};
use wsp_wire::views::golden_image::{GoldenManifest, GoldenStageEvent, RecipeDigest, SealedImage};
use wsp_wire::views::place_more::PlaceView;
use wsp_words::exit::HostError;

use super::engine::{GoldenRoads, KillConfirm};
use super::{Builders, Joined};

/// The place door calls the image areas make (TS `ctx.placeDoor`, places.ts `PlaceDoor`); the join implements it over
/// `places::types::PlaceDoor`.
#[async_trait(?Send)]
pub trait ImageDoor {
    fn backend_of(&self, place_id: &str) -> Option<Arc<dyn MachineBackend>>;
    async fn forking_backend(&self, place_id: &str) -> Result<Arc<dyn MachineBackend>, HostError>;
    /// `nameOf`; none where the door names no such place.
    fn name_of(&self, place_id: &str) -> Option<String>;
    /// `placeFor(word).placeId`.
    async fn place_for(&self, word: &str) -> Result<Option<String>, HostError>;
    /// `defaultPlace().placeId`.
    async fn default_place(&self) -> Result<Option<String>, HostError>;
    async fn mark_default_if_none(&self, place_id: &str) -> Result<(), HostError>;
    async fn list(&self, now: i64) -> Result<Vec<PlaceView>, HostError>;
}

/// The backend `claiming` hands its run: creates through it are held in `inflight` until the claim is dropped, which
/// is when the TS run settles (machines.ts:185-227).
pub trait Claim {
    fn backend(&self) -> Arc<dyn MachineBackend>;
}

/// One place an image can be built at (builders.ts:250).
#[derive(Clone)]
pub struct BuildPlace {
    pub place: String,
    pub name: String,
    pub kind: wsp_wire::views::place_kind::PlaceKind,
    pub backend: Arc<dyn MachineBackend>,
}

/// `ctx.stageAt`'s entry: a build's last stage on a computer (internal.ts).
#[derive(Clone, Debug, PartialEq)]
pub struct StageAt {
    pub place: String,
    pub name: String,
    pub named: bool,
    pub frame: StageFrame,
}

/// What `opts` holds that the builders area reads.
#[derive(Clone, Default)]
pub struct BuildersOpts {
    pub no_machines_line: Option<String>,
    pub golden_recipe: Option<GoldenRecipe>,
    pub copy_recipe: Option<CopyRecipe>,
    pub kill_confirm: Option<KillConfirm>,
    pub snapshot_retry_ms: Option<u64>,
}

/// `opts.copyRecipe`: how a copy is planned off the record.
pub type CopyRecipe = Rc<dyn Fn(SealedImage, bool) -> LocalBoxFuture<'static, Result<GoldenRecipe, HostError>>>;

/// Every call the builders area makes on the rest of the runtime.
#[async_trait(?Send)]
pub trait BuildersCtx {
    fn store(&self) -> Rc<dyn Store>;
    fn clock(&self) -> Rc<dyn Clock>;
    fn roads(&self) -> Rc<dyn GoldenRoads>;
    fn opts(&self) -> BuildersOpts;
    fn places(&self) -> Rc<dyn PlaceBackends>;
    /// `ctx.placeDoor`.
    fn door(&self) -> Option<Rc<dyn ImageDoor>>;
    /// `ctx.placeDoorOf()`: the door, or its absence refused.
    fn place_door_of(&self) -> Result<Rc<dyn ImageDoor>, HostError>;
    /// `ctx.backend`: the runtime's own backend.
    fn backend(&self) -> Arc<dyn MachineBackend>;
    /// `ctx.backendOfKind("cloud", placeId)`.
    fn backend_of_kind_cloud(&self, place_id: Option<&str>) -> Arc<dyn MachineBackend>;
    /// `opts.broker?.ready() === true`.
    fn broker_ready(&self) -> bool;
    fn host_id(&self) -> String;
    /// `process.pid`.
    fn pid(&self) -> u32;
    /// `os.hostname()`.
    fn hostname(&self) -> String;
    /// `ctx.state.owner`.
    fn owner(&self) -> String;
    /// `ctx.state.closed`.
    fn closed(&self) -> bool;
    /// `ctx.state.beat`: the armed heartbeat's cancel.
    fn beat(&self) -> Option<Cancel>;
    fn set_beat(&self, beat: Option<Cancel>);
    /// `ctx.state.ticking = tick()`: the beat's run, spawned.
    fn set_ticking(&self, ticking: LocalBoxFuture<'static, ()>);
    fn warn(&self, line: &str);
    /// `bus.emit` of a `golden.stage` event.
    fn emit_stage(&self, event: GoldenStageEvent);
    /// Runs a detached task on the host's thread (a TS `void p`).
    fn spawn(&self, f: LocalBoxFuture<'static, ()>);
    fn builders(&self) -> Builders;
    /// `ctx.stageAt`.
    fn stage_at(&self) -> Rc<RefCell<IndexMap<String, StageAt>>>;
    /// `ctx.imageMark()`.
    fn image_mark(&self) -> String;
    /// `ctx.refreshBuilders()` (the boot area).
    async fn refresh_builders(&self) -> Result<(), HostError>;
    /// `gone.stop(at, machine)` (the machines area's gone confirmation).
    async fn gone_stop(&self, at: Arc<dyn MachineBackend>, machine: Arc<dyn Machine>) -> Result<(), HostError>;
    /// `ctx.claiming(purpose, .., at, place)`.
    fn claiming(&self, purpose: &str, at: Option<Arc<dyn MachineBackend>>, place: Option<&str>) -> Rc<dyn Claim>;
    /// `ctx.observed(machine)`.
    fn observed(&self, machine: Arc<dyn Machine>) -> Arc<dyn Machine>;
    /// `ctx.observing(backend)`.
    fn observing(&self, backend: Arc<dyn MachineBackend>) -> Arc<dyn MachineBackend>;
    /// The records area.
    async fn record_of(&self, name: &str) -> Result<Option<SealedImage>, HostError>;
    async fn copy_of(&self, place: &str, name: &str) -> Result<Option<GoldenManifest>, HostError>;
    async fn put_copy(&self, place: &str, name: &str, manifest: &GoldenManifest) -> Result<(), HostError>;
    async fn put_copy_recipe(&self, place: &str, name: &str, version: u32, digest: &RecipeDigest) -> Result<(), HostError>;
}

/// `buildersArea(ctx)` (builders.ts:55). Memory only: the grace timers and the one pass of `expire_grace`.
pub struct BuildersArea {
    pub ctx: Rc<dyn BuildersCtx>,
    /// `graceTimers`: one cancel per kept builder.
    pub grace_timers: Rc<RefCell<IndexMap<String, Cancel>>>,
    /// `expiring`: the pass in flight.
    pub expiring: RefCell<Option<Joined<life::Expired>>>,
}

impl BuildersArea {
    pub fn new(ctx: Rc<dyn BuildersCtx>) -> Rc<BuildersArea> {
        todo!()
    }
}
