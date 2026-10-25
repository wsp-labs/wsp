// SPDX-License-Identifier: AGPL-3.0-only
//! `images/held.ts` at 65f6b89c7: the snapshots this host's records say it saved at one place, and the drop of one's
//! record once the provider deleted it. Spec: `spec/images_records.md` (H1, H2).
use std::rc::Rc;

use async_trait::async_trait;

use wsp_core::types::pending::Store;
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
    async fn drop_copy_recipe(&self, place: &str, name: &str, version: u32) -> Result<(), HostError>;
}

/// `heldImages(ctx)` (held.ts:9).
pub struct HeldImages {
    pub ctx: Rc<dyn HeldCtx>,
}

impl HeldImages {
    pub fn new(ctx: Rc<dyn HeldCtx>) -> HeldImages {
        todo!()
    }

    /// held.ts:21-25 (H1).
    pub async fn on(&self, place_id: &str) -> Result<Vec<HeldImage>, HostError> {
        todo!()
    }

    /// held.ts:26-34 (H2).
    pub async fn drop(&self, place_id: &str, id: &str) -> Result<(), HostError> {
        todo!()
    }
}
