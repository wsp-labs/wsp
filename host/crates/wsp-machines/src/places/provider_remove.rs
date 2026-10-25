// SPDX-License-Identifier: AGPL-3.0-only
//! `packages/runtime/src/places/provider-remove.ts` at 65f6b89c7: takes a provider back out, every machine and image
//! this host made there read gone before the key is forgotten. Spec: `spec/places_provider_remove.md`.

use std::rc::Rc;
use std::sync::Arc;

use futures::future::LocalBoxFuture;
use wsp_engine::machine::MachineBackend;
use wsp_wire::views::place_more::PlaceRemoved;

use super::types::{HeldImage, KillConfirm, PlaceKeyForgetter, PlaceResult};

/// How many deletes run at once (provider-remove.ts:141).
pub const KILLS_AT_ONCE: usize = 4;

/// How many times the listing is read again after the deletes (provider-remove.ts:144).
pub const RELISTS: usize = 3;

/// The image records this host holds at the provider, and the drop of one once its snapshot is gone.
pub struct ProviderImages {
    pub held: Rc<dyn Fn() -> LocalBoxFuture<'static, PlaceResult<Vec<HeldImage>>>>,
    pub drop: Rc<dyn Fn(String) -> LocalBoxFuture<'static, PlaceResult<()>>>,
}

/// What a provider's remove is handed (provider-remove.ts:152).
pub struct ProviderRemove {
    pub place_id: String,
    pub backend: Arc<dyn MachineBackend>,
    /// This host's owner mark.
    pub owner: String,
    /// The machine ids this host's own records hold there.
    pub known: Vec<String>,
    pub forget_key: Option<Rc<dyn PlaceKeyForgetter>>,
    pub confirm: Option<KillConfirm>,
    pub images: Option<ProviderImages>,
}

/// Deletes every machine this host made there, then every image snapshot, and only then forgets the key
/// (provider-remove.ts:152).
pub async fn remove_provider(o: ProviderRemove) -> PlaceResult<PlaceRemoved> {
    let _ = o;
    todo!()
}
