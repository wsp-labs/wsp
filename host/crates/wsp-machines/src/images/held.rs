// SPDX-License-Identifier: AGPL-3.0-only
//! `images/held.ts` at 65f6b89c7: the snapshots this host's records say it saved at one place, and the drop of one's
//! record once the provider deleted it. Spec: `spec/images_records.md` (H1, H2).
use std::rc::Rc;

use async_trait::async_trait;
use serde_json::Value;

use wsp_core::types::internal::{copy_key_parts, GOLDENS};
use wsp_core::types::pending::Store;
use wsp_wire::views::golden_image::{GoldenManifest, GoldenVersion};
use wsp_words::exit::HostError;

use crate::places::types::HeldImage;

/// What `heldImages` reads off `ctx`.
#[async_trait(?Send)]
pub trait HeldCtx {
    fn store(&self) -> Rc<dyn Store>;
    /// `places.wired`.
    fn wired(&self) -> String;
    /// `ctx.ready()`.
    async fn ready(&self) -> Result<(), HostError>;
    /// `ctx.dropCopyRecipe` (the records area).
    async fn drop_copy_recipe(
        &self,
        place: &str,
        name: &str,
        version: u32,
    ) -> Result<(), HostError>;
}

/// `heldImages(ctx)` (held.ts:9).
pub struct HeldImages {
    pub ctx: Rc<dyn HeldCtx>,
}

/// One stored copy of a golden at a place: its key, its name and its manifest as the store holds it.
struct Copy {
    key: String,
    name: String,
    raw: Value,
    manifest: GoldenManifest,
}

impl HeldImages {
    pub fn new(ctx: Rc<dyn HeldCtx>) -> HeldImages {
        HeldImages { ctx }
    }

    async fn copies_at(&self, place_id: &str) -> Result<Vec<Copy>, HostError> {
        let store = self.ctx.store();
        let mut copies = Vec::new();
        for key in store.keys(GOLDENS).await? {
            let parts = copy_key_parts(&key);
            if parts.place.unwrap_or_else(|| self.ctx.wired()) != place_id {
                continue;
            }
            let Some(raw) = store.get(GOLDENS, &key).await? else {
                continue;
            };
            let Ok(manifest) = serde_json::from_value::<GoldenManifest>(raw.clone()) else {
                continue;
            };
            copies.push(Copy {
                key,
                name: parts.name,
                raw,
                manifest,
            });
        }
        Ok(copies)
    }

    /// held.ts:21-25 (H1).
    pub async fn on(&self, place_id: &str) -> Result<Vec<HeldImage>, HostError> {
        self.ctx.ready().await?;
        let held = self
            .copies_at(place_id)
            .await?
            .into_iter()
            .flat_map(|c| {
                c.manifest.versions.into_iter().map(|v| HeldImage {
                    id: v.snapshot_id,
                    template_id: v.template_id,
                })
            })
            .collect();
        Ok(held)
    }

    /// held.ts:26-34 (H2).
    pub async fn drop(&self, place_id: &str, id: &str) -> Result<(), HostError> {
        let store = self.ctx.store();
        for copy in self.copies_at(place_id).await? {
            let (gone, kept): (Vec<GoldenVersion>, Vec<GoldenVersion>) = copy
                .manifest
                .versions
                .into_iter()
                .partition(|v| v.snapshot_id == id);
            if gone.is_empty() {
                continue;
            }
            if kept.is_empty() {
                store.delete(GOLDENS, &copy.key).await?;
            } else {
                let mut manifest = copy.raw;
                let versions = serde_json::to_value(&kept).expect("a version serializes as JSON");
                if let Some(obj) = manifest.as_object_mut() {
                    obj.insert("versions".to_string(), versions);
                }
                store.put(GOLDENS, &copy.key, manifest).await?;
            }
            for v in gone {
                self.ctx
                    .drop_copy_recipe(place_id, &copy.name, v.version.0 as u32)
                    .await?;
            }
        }
        Ok(())
    }
}
