// SPDX-License-Identifier: AGPL-3.0-only
//! golden.ts:66-72, 461-611 at 65f6b89c7: storage, orphans, deleteOrphans, retentionPlaces, retention, prune,
//! rollback and promote. Spec: `spec/golden_storage.md`.
use std::sync::Arc;

use wsp_core::types::api::{AccountOrphans, OrphansDeleted, PlacedRetention, Pruned};
use wsp_core::types::wiring::GoldenPromotion;
use wsp_engine::machine::MachineBackend;
use wsp_wire::views::golden_image::GoldenManifest;
use wsp_wire::views::snapshot_lineage::SnapshotStorage;
use wsp_words::exit::HostError;

use super::GoldenArea;
use crate::images::ImageResult;

impl GoldenArea {
    /// golden.ts:68 (S1): `Some([])` where the runtime backend bills named snapshots by count.
    pub fn account_ids(&self) -> Option<Vec<String>> {
        todo!()
    }

    /// golden.ts:69-72 (S2).
    pub async fn storage_at(&self, at: Arc<dyn MachineBackend>, ids: Option<Vec<String>>) -> ImageResult<Option<SnapshotStorage>> {
        todo!()
    }

    /// golden.ts:461-464 (S2).
    pub async fn storage(&self) -> ImageResult<Option<SnapshotStorage>> {
        todo!()
    }

    /// golden.ts:466-486 (S3).
    pub async fn orphans(&self) -> ImageResult<Option<AccountOrphans>> {
        todo!()
    }

    /// golden.ts:488-519 (S4).
    pub async fn delete_orphans(&self) -> ImageResult<Option<OrphansDeleted>> {
        todo!()
    }

    /// golden.ts:521-533 (S5).
    pub async fn retention_places(&self, name: Option<&str>) -> Result<Vec<String>, HostError> {
        todo!()
    }

    /// golden.ts:535-547 (S6).
    pub async fn retention(&self, name: Option<&str>, asked: Option<&str>) -> ImageResult<Option<PlacedRetention>> {
        todo!()
    }

    /// golden.ts:549-574 (S7).
    pub async fn prune(&self, name: Option<&str>, asked: Option<&str>) -> ImageResult<Pruned> {
        todo!()
    }

    /// golden.ts:576-589 (S8).
    pub async fn rollback(&self, version: u32, name: Option<&str>) -> Result<GoldenManifest, HostError> {
        todo!()
    }

    /// golden.ts:591-611 (S9).
    pub async fn promote(&self, name: Option<&str>) -> Result<Option<Vec<GoldenPromotion>>, HostError> {
        todo!()
    }
}
