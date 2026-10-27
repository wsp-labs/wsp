// SPDX-License-Identifier: AGPL-3.0-only
//! Packet 3's roads: the provider backends and the places over them, the place table's `provider` and `forget_key` roads,
//! and the live key check the broker door answers `providers.checkKey` with. A host with no provider key wires none.
use std::rc::Rc;
use std::sync::Arc;

use wsp_core::types::wiring::PlaceBackends;
use wsp_engine::machine::MachineBackend;
use wsp_machines::places::types::{PlaceKeyForgetter, WiredProvider};
use wsp_wire::wire::runtime::ProviderKeyWorks;
use wsp_words::exit::HostError;

use crate::context::HostWiring;

/// `providerBackendFor(env, modules)` (`cli/wiring.ts`): the backend the provider slot starts on, where a key is saved.
pub fn provider_backend(_wiring: &HostWiring) -> Option<Arc<dyn MachineBackend>> {
    None
}

/// `providerPlaces(...)` (`cli/wiring.ts`, `providers.ts`): the table the runtime resolves a place name through.
pub fn provider_places(_wiring: &HostWiring, _backend: &Arc<dyn MachineBackend>) -> Option<Rc<dyn PlaceBackends>> {
    None
}

/// `PlaceWiring.provider`: `wiredPlaceRow(pick.env, slot.current(), modules)`.
pub fn wired_provider(_wiring: &HostWiring) -> Option<Rc<dyn Fn() -> Option<WiredProvider>>> {
    None
}

/// `PlaceWiring.forgetKey`: `forgetProviderKey(rt, statePath, placeId, keyReads)`.
pub fn forget_key(_wiring: &HostWiring) -> Option<Rc<dyn PlaceKeyForgetter>> {
    None
}

/// `checkProviderKeyLive(provider, key)` (`cli/serve.ts:289`), the broker door's `check_key`: `None` is a host that wired
/// no check.
pub async fn check_provider_key_live(_provider: &str, _key: &str) -> Option<Result<ProviderKeyWorks, HostError>> {
    None
}
