// SPDX-License-Identifier: AGPL-3.0-only
//! golden.ts:64-290 at 65f6b89c7: build, get, buildPlace, prepareAt, prepare, seal and recipe. Spec:
//! `spec/golden_prepare.md`.
use std::rc::Rc;

use wsp_core::types::api::{BuildPlace, PrepareAsk, Sealed};
use wsp_core::types::wiring::GoldenBuildRequest;
use wsp_wire::views::golden_image::{GoldenBuilderView, GoldenLogin, GoldenManifest, RecipeDigest};
use wsp_words::exit::HostError;

use super::GoldenArea;
use crate::images::builders::places::PlaceAt;
use crate::images::ImageResult;

/// `prepareAt`'s answer: the place and what a sentence calls it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PreparedAt {
    pub place: String,
    pub name: String,
}

impl GoldenArea {
    /// golden.ts:80-97 (G1).
    pub async fn build(self: &Rc<Self>, o: GoldenBuildRequest) -> ImageResult<Sealed> {
        todo!()
    }

    /// golden.ts:98-102 (G2).
    pub async fn get(&self, name: Option<&str>) -> Result<Option<GoldenManifest>, HostError> {
        todo!()
    }

    /// golden.ts:104-131 (G3).
    pub async fn build_place(&self, word: Option<&str>) -> ImageResult<BuildPlace> {
        todo!()
    }

    /// golden.ts:133-137 (G4).
    pub async fn prepare_at(&self, name: Option<&str>, place: Option<&str>) -> Result<PreparedAt, HostError> {
        todo!()
    }

    /// golden.ts:75-78: the place a prepare of `name` runs at.
    pub async fn place_for_prepare(&self, name: &str, word: Option<&str>, copy: bool) -> Result<PlaceAt, HostError> {
        todo!()
    }

    /// golden.ts:139-271 (G5 to G9).
    pub async fn prepare(self: &Rc<Self>, o: PrepareAsk) -> ImageResult<GoldenBuilderView> {
        todo!()
    }

    /// golden.ts:273-281 (G10).
    pub async fn seal(&self, builder_id: &str, logins: Option<Vec<GoldenLogin>>, keep_builder: Option<bool>) -> ImageResult<Sealed> {
        todo!()
    }

    /// golden.ts:283-290 (G11).
    pub async fn recipe(&self, name: Option<&str>) -> Result<Option<RecipeDigest>, HostError> {
        todo!()
    }
}
