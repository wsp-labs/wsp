// SPDX-License-Identifier: AGPL-3.0-only
//! builders.ts:57-78, 190-281 at 65f6b89c7: where an image is built, and how a builder is shown. Spec:
//! `spec/builders_places.md`.
use std::sync::Arc;

use indexmap::IndexMap;

use wsp_core::types::internal::BuilderRecord;
use wsp_core::types::wiring::GoldenRecipe;
use wsp_engine::machine::MachineBackend;
use wsp_wire::views::golden_image::GoldenBuilderView;
use wsp_wire::views::place_kind::PlaceKind;
use wsp_words::exit::HostError;

use super::{BuildPlace, BuildersArea, CopyRecipe};
use crate::images::LiveBuilder;

/// `placeAt`'s answer.
#[derive(Clone)]
pub struct PlaceAt {
    pub place: String,
    pub at: Arc<dyn MachineBackend>,
}

impl BuildersArea {
    /// builders.ts:57-78 (P1).
    pub fn builder_view(&self, r: &BuilderRecord, b: &LiveBuilder) -> GoldenBuilderView {
        todo!()
    }

    /// builders.ts:191-195 (P2).
    pub fn backend_at(&self, place: &str) -> Result<Arc<dyn MachineBackend>, HostError> {
        todo!()
    }

    /// builders.ts:200-221 (P3).
    pub async fn place_at(&self, word: &str) -> Result<PlaceAt, HostError> {
        todo!()
    }

    /// builders.ts:225-230 (P5).
    pub async fn landing_backend(&self, place_id: Option<&str>) -> Result<Arc<dyn MachineBackend>, HostError> {
        todo!()
    }

    /// builders.ts:233-239 (P4).
    pub async fn forking_at(&self, place_id: Option<&str>) -> Result<Option<Arc<dyn MachineBackend>>, HostError> {
        todo!()
    }

    /// builders.ts:242 (P6).
    pub fn place_name(&self, place: &str) -> String {
        todo!()
    }

    /// builders.ts:245 (P7).
    pub async fn image_place(&self, name: &str) -> Result<String, HostError> {
        todo!()
    }

    /// builders.ts:247 (P8).
    pub fn place_kind_of(&self, place: &str) -> PlaceKind {
        todo!()
    }

    /// builders.ts:250-263 (P9).
    pub async fn build_places(&self) -> Result<Vec<BuildPlace>, HostError> {
        todo!()
    }

    /// builders.ts:267-271 (P10).
    pub fn copy_recipe_or_throw(&self) -> Result<CopyRecipe, HostError> {
        todo!()
    }

    /// builders.ts:275-279 (P10).
    pub fn recipe_or_throw(&self, named: Option<GoldenRecipe>) -> Result<GoldenRecipe, HostError> {
        todo!()
    }

    /// builders.ts:281 (P11).
    pub fn builder_labels(&self, extra: Option<&IndexMap<String, String>>) -> IndexMap<String, String> {
        todo!()
    }
}
