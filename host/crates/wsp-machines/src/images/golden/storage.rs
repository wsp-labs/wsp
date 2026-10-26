// SPDX-License-Identifier: AGPL-3.0-only
//! golden.ts:66-72, 461-611 at 65f6b89c7: storage, orphans, deleteOrphans, retentionPlaces, retention, prune,
//! rollback and promote. Spec: `spec/golden_storage.md`.
use std::collections::HashSet;
use std::rc::Rc;
use std::sync::Arc;

use indexmap::IndexSet;

use wsp_core::types::api::{AccountOrphans, AccountRows, OrphanFailed, OrphansDeleted, PlacedRetention, Pruned};
use wsp_core::types::internal::{copy_key_parts, GOLDENS};
use wsp_core::types::wiring::{GoldenPromotion, PromotionDone};
use wsp_engine::errors::is_missing;
use wsp_engine::machine::MachineBackend;
use wsp_engine::snapshot_names::golden_name;
use wsp_engine::snapshot_storage::{billed_by_count, named_count, retention_plan, snapshot_monthly_usd, snapshot_storage, split_by_owner, OwnerRead};
use wsp_wire::views::golden_image::GoldenManifest;
use wsp_wire::views::snapshot_lineage::SnapshotStorage;
use wsp_wire::wire::machine_link::TemplateRow;
use wsp_words::exit::{HostError, Kind};

use super::super::engine::Templates;
use super::super::ImageError;
use super::super::ImageResult;
use super::GoldenArea;

impl GoldenArea {
    /// golden.ts:68 (S1): `Some([])` where the runtime backend bills named snapshots by count.
    pub fn account_ids(&self) -> Option<Vec<String>> {
        billed_by_count(&self.ctx.backend().pricing().snapshot_storage()).then(Vec::new)
    }

    /// golden.ts:69-72 (S2).
    pub async fn storage_at(&self, at: Arc<dyn MachineBackend>, ids: Option<Vec<String>>) -> ImageResult<Option<SnapshotStorage>> {
        if !at.capabilities().snapshot_listing {
            return Ok(None);
        }
        let rows = at.list_snapshots(ids.as_deref()).await.map_err(ImageError::Engine)?;
        let read = self.owner_read().await?;
        let pricing = at.pricing().snapshot_storage();
        snapshot_storage(&rows, &pricing, &read).map(Some).map_err(ImageError::Engine)
    }

    /// golden.ts:461-464 (S2).
    pub async fn storage(&self) -> ImageResult<Option<SnapshotStorage>> {
        self.ctx.ready().await?;
        let backend = self.ctx.backend();
        self.storage_at(backend, self.account_ids()).await
    }

    /// golden.ts:466-486 (S3).
    pub async fn orphans(&self) -> ImageResult<Option<AccountOrphans>> {
        self.ctx.ready().await?;
        let backend = self.ctx.backend();
        if !backend.capabilities().snapshot_listing {
            return Ok(None);
        }
        let read = self.owner_read().await?;
        let rows = backend.list_snapshots(self.account_ids().as_deref()).await.map_err(ImageError::Engine)?;
        let snapshots = split_by_owner(&rows, &read).map_err(ImageError::Engine)?;
        let listed: Vec<TemplateRow> = match self.ctx.roads().templates_of(backend.clone()) {
            Some(templates) => templates.list().await.map_err(ImageError::Engine)?,
            None => vec![],
        };
        let promoted = split_by_owner(&listed, &read).map_err(ImageError::Engine)?;
        let total_bytes: f64 = rows.iter().map(|r| r.size_bytes.0).sum();
        let freed_bytes: f64 = snapshots.orphans.iter().map(|r| r.size_bytes.0).sum();
        let pricing = backend.pricing().snapshot_storage();
        let named = named_count(&rows);
        let named_orphans = named_count(&snapshots.orphans);
        let saves = snapshot_monthly_usd(total_bytes, &pricing, named) - snapshot_monthly_usd(total_bytes - freed_bytes, &pricing, named.saturating_sub(named_orphans));
        Ok(Some(AccountOrphans {
            snapshots: snapshots.orphans,
            templates: promoted.orphans,
            freed_bytes,
            saves_usd_per_month: saves,
            others: AccountRows { snapshots: snapshots.foreign, templates: promoted.foreign },
        }))
    }

    /// golden.ts:488-519 (S4).
    pub async fn delete_orphans(&self) -> ImageResult<Option<OrphansDeleted>> {
        let Some(plan) = self.orphans().await? else {
            return Ok(None);
        };
        let backend = self.ctx.backend();
        let mut deleted = OrphansDeleted { snapshots: vec![], templates: vec![], failed: vec![] };
        if let Some(templates) = self.ctx.roads().templates_of(backend.clone()) {
            for row in plan.templates {
                match templates.delete(&row.id).await {
                    Ok(()) => deleted.templates.push(row),
                    Err(e) => deleted.failed.push(OrphanFailed { id: row.id.clone(), name: Some(row.name.clone()), message: e.to_string() }),
                }
            }
        }
        for row in plan.snapshots {
            match backend.delete_snapshot(&row.id).await {
                Ok(()) => deleted.snapshots.push(row),
                Err(e) if is_missing(&e) => deleted.snapshots.push(row),
                Err(e) => deleted.failed.push(OrphanFailed { id: row.id.clone(), name: row.name.clone(), message: e.to_string() }),
            }
        }
        Ok(Some(deleted))
    }

    /// golden.ts:521-533 (S5).
    pub async fn retention_places(&self, name: Option<&str>) -> Result<Vec<String>, HostError> {
        self.ctx.ready().await?;
        let key = name.unwrap_or("default");
        let own = self.ctx.image_place(key).await?;
        let wired = self.ctx.places().wired();
        let mut held: IndexSet<String> = IndexSet::new();
        for stored in self.ctx.store().keys(GOLDENS).await? {
            let parts = copy_key_parts(&stored);
            if parts.name == key {
                held.insert(parts.place.unwrap_or_else(|| wired.clone()));
            }
        }
        let mut others: Vec<String> = held
            .iter()
            .filter(|p| **p != own && **p != wired && self.backend_of_place(p.as_str()).is_some())
            .cloned()
            .collect();
        others.sort_by(|a, b| a.encode_utf16().cmp(b.encode_utf16()));
        let mut out = vec![own.clone()];
        if own != wired && held.contains(&wired) {
            out.push(wired);
        }
        out.extend(others);
        Ok(out)
    }

    /// golden.ts:535-547 (S6).
    pub async fn retention(&self, name: Option<&str>, asked: Option<&str>) -> ImageResult<Option<PlacedRetention>> {
        self.ctx.ready().await?;
        let key = name.unwrap_or("default");
        let own = self.ctx.image_place(key).await?;
        let place = asked.map_or_else(|| own.clone(), str::to_string);
        let at = self.ctx.backend_at(&place)?;
        if !at.capabilities().snapshot_listing {
            return Ok(None);
        }
        let Some(manifest) = self.ctx.copy_of(&place, key).await? else {
            return Ok(None);
        };
        let ids: Vec<String> = manifest.versions.iter().map(|v| v.snapshot_id.clone()).collect();
        let rows = at.list_snapshots(Some(&ids)).await.map_err(ImageError::Engine)?;
        let ctx = self.ctx.clone();
        let forked = move |snapshot: &str| ctx.forked_from(snapshot);
        let pricing = at.pricing().snapshot_storage();
        let plan = retention_plan(&manifest, &rows, &forked, &pricing, None);
        Ok(Some(PlacedRetention { plan, place_name: self.ctx.place_name(&place), copy: place != own, place }))
    }

    /// golden.ts:549-574 (S7).
    pub async fn prune(&self, name: Option<&str>, asked: Option<&str>) -> ImageResult<Pruned> {
        let key = name.unwrap_or("default");
        let Some(plan) = self.retention(Some(key), asked).await? else {
            return Ok(Pruned { dropped: vec![], failed: vec![], storage: None });
        };
        let place = plan.place.clone();
        let at = self.ctx.backend_at(&place)?;
        let mut dropped = Vec::new();
        let mut failed = Vec::new();
        for v in plan.plan.drop.iter() {
            match self.ctx.drop_image(v, Some(at.clone())).await {
                Ok(()) => {}
                Err(e) if e.is_missing() => {}
                Err(e) => {
                    failed.push((v.version.0 as u32, e.message()));
                    continue;
                }
            }
            let Some(mut manifest) = self.ctx.copy_of(&place, key).await? else {
                return Err(HostError::with_kind(format!("no golden named \"{key}\""), Kind::Missing).into());
            };
            manifest.versions.retain(|x| x.version.0 != v.version.0);
            self.ctx.put_copy(&place, key, &manifest).await?;
            self.ctx.drop_copy_recipe(&place, key, v.version.0 as u32).await?;
            dropped.push(v.clone());
        }
        let keep: Vec<String> = plan.plan.keep.iter().map(|v| v.snapshot_id.clone()).collect();
        let storage = self.storage_at(at, Some(keep)).await.ok().flatten();
        Ok(Pruned { dropped, failed, storage })
    }

    /// golden.ts:576-589 (S8). Does not wait for ready.
    pub async fn rollback(&self, version: u32, name: Option<&str>) -> Result<GoldenManifest, HostError> {
        let key = name.unwrap_or("default");
        let wired = self.ctx.places().wired();
        let Some(prior) = self.ctx.copy_of(&wired, key).await? else {
            return Err(HostError::with_kind(format!("no golden named \"{key}\""), Kind::Missing));
        };
        let next = self.ctx.roads().rollback(&prior, version).map_err(|e| HostError::with_kind(e.to_string(), Kind::Missing))?;
        self.ctx.put_copy(&wired, key, &next).await?;
        Ok(next)
    }

    /// golden.ts:591-611 (S9).
    pub async fn promote(&self, name: Option<&str>) -> Result<Option<Vec<GoldenPromotion>>, HostError> {
        self.ctx.ready().await?;
        let backend = self.ctx.backend();
        let Some(templates) = self.ctx.roads().templates_of(backend) else {
            return Ok(None);
        };
        let key = name.unwrap_or("default");
        let wired = self.ctx.places().wired();
        let manifest = self.ctx.copy_of(&wired, key).await?;
        let mut rows = Vec::new();
        for v in manifest.map(|m| m.versions).unwrap_or_default() {
            if v.template_id.is_some() {
                continue;
            }
            let done = match self.promote_one(&templates, key, &wired, v.snapshot_id.clone(), v.version.0 as u64, v.version.0 as u32).await {
                Ok((template_id, sharing)) => PromotionDone::Promoted { template_id, sharing },
                Err(error) => PromotionDone::Failed { error },
            };
            rows.push(GoldenPromotion { golden: key.to_string(), version: v.version.0 as u32, done });
        }
        Ok(Some(rows))
    }

    fn backend_of_place(&self, place: &str) -> Option<Arc<dyn MachineBackend>> {
        self.ctx.places().backend(place).or_else(|| self.ctx.door().and_then(|d| d.backend_of(place)))
    }

    /// The owner read a storage listing is judged by (golden.ts:70, 480).
    async fn owner_read(&self) -> Result<OwnerRead, HostError> {
        let recorded: HashSet<String> = self.ctx.recorded_images().await?.into_iter().collect();
        Ok(OwnerRead { host_id: self.ctx.image_mark(), recorded, now: self.ctx.clock().now() })
    }

    /// golden.ts:553-562 (S9): one version's template, its wired copy re-read and written back.
    async fn promote_one(
        &self,
        templates: &Rc<dyn Templates>,
        key: &str,
        wired: &str,
        snapshot_id: String,
        version: u64,
        version_number: u32,
    ) -> Result<(String, Option<u32>), String> {
        let name = golden_name(&self.ctx.image_mark(), key, version).map_err(|e| e.to_string())?;
        let promoted = self.ctx.roads().promote_version(templates.clone(), &snapshot_id, &name).await.map_err(|e| e.to_string())?;
        let Some(mut current) = self.ctx.copy_of(wired, key).await.map_err(|e| e.message)? else {
            return Err(format!("golden {key} was dropped while its versions were being promoted"));
        };
        for x in current.versions.iter_mut() {
            if x.version.0 as u32 == version_number {
                x.template_id = Some(promoted.template_id.clone());
            }
        }
        self.ctx.put_copy(wired, key, &current).await.map_err(|e| e.message)?;
        Ok((promoted.template_id, promoted.sharing))
    }
}
