// SPDX-License-Identifier: AGPL-3.0-only
//! builders.ts:80-189, 283-472 at 65f6b89c7: a builder's hold and heartbeat, its grace, its stages, the recording
//! create, the settle and the seal. Spec: `spec/builders_life.md`.
use std::rc::Rc;
use std::sync::Arc;

use wsp_engine::machine::MachineBackend;
use wsp_engine::orphans::{ReapFailure, ReapedMachine};
use wsp_wire::views::golden_image::{GoldenLogin, GoldenVersion, SealedImage};
use wsp_words::exit::HostError;

use super::BuildersArea;
use crate::images::engine::{EngineBuilder, ImportLedger, SealResult, Signal, StageFn};
use crate::images::{BuilderRef, ImageResult};
use wsp_core::types::wiring::GoldenRecipe;

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

/// `recordingCreates`' stop: the prepare's signal and where it hands the create in flight.
#[derive(Clone)]
pub struct CreateStop {
    pub signal: Option<Signal>,
    /// Handed the creating future before the provider answers (TS `began(creating)`).
    pub began: Rc<dyn Fn(futures::future::Shared<futures::future::LocalBoxFuture<'static, ImageResult<()>>>)>,
}

impl BuildersArea {
    /// builders.ts:82-89 (L1).
    pub fn arm_grace(self: &Rc<Self>, id: &str, sealed_at: &str) {
        todo!()
    }

    /// builders.ts:91-94 (L2).
    pub fn in_window(&self, sealed_at: &str) -> bool {
        todo!()
    }

    /// builders.ts:99-118 (L3, L4).
    pub async fn expire_grace(self: &Rc<Self>) -> ImageResult<Expired> {
        todo!()
    }

    /// builders.ts:141-147 (L5).
    pub async fn hold(self: &Rc<Self>, b: &BuilderRef) -> Result<(), HostError> {
        todo!()
    }

    /// builders.ts:119-129 (L6): arms one heartbeat.
    pub fn arm(self: &Rc<Self>) {
        todo!()
    }

    /// builders.ts:131-137 (L6): one beat.
    pub async fn tick(self: &Rc<Self>) {
        todo!()
    }

    /// builders.ts:151-155 (L7).
    pub fn refuse_untouchable(&self, entry: &BuilderRef) -> Result<(), HostError> {
        todo!()
    }

    /// builders.ts:157-160 (L8).
    pub async fn forget_builder(&self, id: &str) -> Result<(), HostError> {
        todo!()
    }

    /// builders.ts:166-172 (L9).
    pub async fn forget_if_gone(&self, entry: &BuilderRef, at: Arc<dyn MachineBackend>) -> Result<(), HostError> {
        todo!()
    }

    /// builders.ts:177-189 (L10).
    pub fn stage_of(&self, name: &str, on: Option<&str>, named: bool) -> StageFn {
        todo!()
    }

    /// builders.ts:285-324 (L11): a backend that is `b` but for `create`.
    pub fn recording_creates(
        self: &Rc<Self>,
        b: Arc<dyn MachineBackend>,
        name: &str,
        imp: Option<ImportMark>,
        made: Rc<dyn Fn(BuilderRef)>,
        stop: Option<CreateStop>,
        place: &str,
    ) -> Arc<dyn MachineBackend> {
        todo!()
    }

    /// builders.ts:327-350 (L12).
    pub async fn settle_builder(self: &Rc<Self>, name: &str, builder: EngineBuilder, placeholder: Option<BuilderRef>, place: &str) -> Result<BuilderRef, HostError> {
        todo!()
    }

    /// builders.ts:357-395 (L13).
    pub async fn record_seal(&self, place: &str, name: &str, entry: &BuilderRef, recipe: &GoldenRecipe, result: SealResult, copy: Option<&SealedImage>) -> Result<SealResult, HostError> {
        todo!()
    }

    /// builders.ts:401-464 (L14 to L16).
    pub async fn seal_entry(self: &Rc<Self>, entry: &BuilderRef, keep: bool, logins: Option<Vec<GoldenLogin>>, copy: Option<SealedImage>) -> ImageResult<SealResult> {
        todo!()
    }

    /// builders.ts:469-472 (L17): `at` none is the runtime backend.
    pub async fn drop_image(&self, v: &GoldenVersion, at: Option<Arc<dyn MachineBackend>>) -> ImageResult<()> {
        todo!()
    }
}

/// The ledger a recording create stamps (builders.ts:312): `{recipeHash, recipe?, applied: [], smoke: "true"}`.
pub fn placeholder_ledger(imp: &ImportMark) -> ImportLedger {
    todo!()
}
