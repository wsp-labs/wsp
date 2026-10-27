// SPDX-License-Identifier: AGPL-3.0-only
//! Port of `providers.ts`: every machine provider this computer can be set up for, as one table of rows.
use std::rc::Rc;
use std::sync::Arc;

use futures::future::LocalBoxFuture;
use indexmap::IndexMap;
use serde_json::Value;
use wsp_core::types::wiring::PlaceBackends;
use wsp_engine::key_check::KeyCheck;
use wsp_engine::machine::MachineBackend;
use wsp_wire::types::JsNumber;
use wsp_wire::wire::runtime::ProviderKeyWorks;
use wsp_words::exit::HostError;

use crate::providers::providers_key::KeyRefusal;

/// `ProviderEnv` (`providers.ts:24`).
pub type ProviderEnv = IndexMap<String, Option<String>>;

/// `ProviderModule` (`providers.ts:26`).
#[derive(Clone)]
pub struct ProviderModule {
    pub id: String,
    pub env_names: Vec<String>,
    pub key_env: Option<String>,
    pub key_name: Option<String>,
    pub key_console: Option<String>,
    pub selects: Rc<dyn Fn(&ProviderEnv) -> bool>,
    pub stands_for: Option<Rc<dyn Fn(&ProviderEnv) -> Option<String>>>,
    pub build: Rc<dyn Fn(&ProviderEnv) -> Arc<dyn MachineBackend>>,
    pub check_key: Option<Rc<dyn Fn(&str) -> LocalBoxFuture<'static, Result<ProviderKeyAnswer, KeyRefusal>>>>,
    pub added_line: Option<Rc<dyn Fn(&Value) -> String>>,
}

/// The inline return of `ProviderModule.checkKey` (`providers.ts:50`), `limits` as the provider's read gave it.
#[derive(Clone, Debug, PartialEq)]
pub struct ProviderKeyAnswer {
    pub works: ProviderKeyWorks,
    pub limits: Option<Value>,
}

/// `PROVIDER_ENV` (`providers.ts:57`).
pub const PROVIDER_ENV: &str = "WSP_PROVIDER";

/// `NO_PROVIDER` (`providers.ts:61`).
pub const NO_PROVIDER: &str = "none";

/// `BOX_KEY_ENV` (`providers.ts:64`).
pub const BOX_KEY_ENV: &str = "BOAT_API_KEY";

/// `SOLARI_KEY_ENV` (`providers.ts:66`).
pub const SOLARI_KEY_ENV: &str = "SOLARI_API_KEY";

/// `providerTable` (`providers.ts:110`).
pub fn provider_table(_cloud: bool) -> Vec<ProviderModule> {
    todo!()
}

/// `PROVIDER_MODULES` (`providers.ts:149`), a constant computed from `providerTable(CLOUD_ON)`.
pub fn provider_modules() -> Vec<ProviderModule> {
    todo!()
}

/// `cloudRoads` (`providers.ts:153`).
pub fn cloud_roads(_modules: &[ProviderModule]) -> bool {
    todo!()
}

/// `noMachinesLine` (`providers.ts:158`).
pub fn no_machines_line(_modules: &[ProviderModule]) -> String {
    todo!()
}

/// `unregisteredCloud` (`providers.ts:162`).
pub fn unregistered_cloud(_word: Option<&str>, _modules: &[ProviderModule]) -> bool {
    todo!()
}

/// `providerEnvNames` (`providers.ts:169`).
pub fn provider_env_names(_modules: &[ProviderModule]) -> Vec<String> {
    todo!()
}

/// `AddedBy`, the `"key"` of `addedBy`'s return (`providers.ts:177`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AddedBy {
    Key,
}

/// `addedBy` (`providers.ts:177`).
pub fn added_by(_m: &ProviderModule) -> Option<AddedBy> {
    todo!()
}

/// `addedProviders` (`providers.ts:183`).
pub fn added_providers(_modules: &[ProviderModule]) -> Vec<ProviderModule> {
    todo!()
}

/// `keyOwner` (`providers.ts:189`).
pub fn key_owner(_variable: &str, _modules: &[ProviderModule]) -> Option<String> {
    todo!()
}

/// `providerKeyEnvs` (`providers.ts:197`).
pub fn provider_key_envs(_modules: &[ProviderModule]) -> Vec<String> {
    todo!()
}

/// `providerModule` (`providers.ts:202`).
pub fn provider_module(_env: &ProviderEnv, _modules: &[ProviderModule]) -> ProviderModule {
    todo!()
}

/// `placeIdOf` (`providers.ts:209`).
pub fn place_id_of(_m: &ProviderModule, _env: &ProviderEnv) -> String {
    todo!()
}

/// `wiredProviderId` (`providers.ts:214`).
pub fn wired_provider_id(_env: &ProviderEnv, _modules: &[ProviderModule]) -> String {
    todo!()
}

/// `providerBackendFor` (`providers.ts:218`).
pub fn provider_backend_for(_env: &ProviderEnv, _modules: &[ProviderModule]) -> Arc<dyn MachineBackend> {
    todo!()
}

/// `namedProviderBackend` (`providers.ts:225`).
pub fn named_provider_backend(
    _env: &ProviderEnv,
    _provider: Option<&str>,
    _modules: &[ProviderModule],
) -> Result<Arc<dyn MachineBackend>, HostError> {
    todo!()
}

/// `isPlace` (`providers.ts:244`).
pub fn is_place(_m: &ProviderModule, _env: &ProviderEnv) -> bool {
    todo!()
}

/// The inline return of `wiredPlaceRow` (`providers.ts:251`).
#[derive(Clone, Debug, PartialEq)]
pub struct WiredPlaceRow {
    pub id: String,
    pub rate_usd_per_hour: JsNumber,
}

/// `wiredPlaceRow` (`providers.ts:251`).
pub fn wired_place_row(_env: &ProviderEnv, _backend: &dyn MachineBackend, _modules: &[ProviderModule]) -> Option<WiredPlaceRow> {
    todo!()
}

/// `placeProviders` (`providers.ts:260`).
pub fn place_providers(_env: &ProviderEnv, _modules: &[ProviderModule]) -> Vec<ProviderModule> {
    todo!()
}

/// `providerPlaces` (`providers.ts:276`).
pub fn provider_places(
    _wired: Rc<dyn Fn() -> String>,
    _backend: Arc<dyn MachineBackend>,
    _env: Rc<dyn Fn() -> ProviderEnv>,
    _modules: &[ProviderModule],
) -> Rc<dyn PlaceBackends> {
    todo!()
}

/// `providerKeyRow` (`providers.ts:309`).
pub fn provider_key_row(_env: &ProviderEnv, _modules: &[ProviderModule]) -> Option<ProviderModule> {
    todo!()
}

/// `keyRowFor` (`providers.ts:317`).
pub fn key_row_for(_env: &ProviderEnv, _provider: Option<&str>, _modules: &[ProviderModule]) -> Option<ProviderModule> {
    todo!()
}

/// `providerKeySet` (`providers.ts:327`).
pub fn provider_key_set(
    _env: &ProviderEnv,
    _key: &str,
    _provider: Option<&str>,
    _modules: &[ProviderModule],
) -> Option<IndexMap<String, String>> {
    todo!()
}

/// `providerKeyRows` (`providers.ts:334`).
pub fn provider_key_rows(_modules: &[ProviderModule]) -> IndexMap<String, String> {
    todo!()
}

/// `providerEnvWithKey` (`providers.ts:341`).
pub fn provider_env_with_key(_env: &ProviderEnv, _key: &str, _provider: Option<&str>, _modules: &[ProviderModule]) -> ProviderEnv {
    todo!()
}

/// `providerKeyCheck` (`providers.ts:349`).
pub async fn provider_key_check(
    _env: &ProviderEnv,
    _key: &str,
    _provider: Option<&str>,
    _modules: &[ProviderModule],
) -> Result<KeyCheck, HostError> {
    todo!()
}

/// `checkProviderKeyLive` (`providers.ts:365`).
pub async fn check_provider_key_live(
    _env: &ProviderEnv,
    _key: &str,
    _provider: &str,
    _modules: &[ProviderModule],
) -> Result<ProviderKeyWorks, KeyRefusal> {
    todo!()
}

/// The `flags` of `providerEnvWith` (`providers.ts:378`), an options object.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ProviderEnvWithOptions {
    pub provider: Option<String>,
}

/// `providerEnvWith` (`providers.ts:378`).
pub fn provider_env_with(
    _flags: &ProviderEnvWithOptions,
    _env: &ProviderEnv,
    _layers: &[ProviderEnv],
    _modules: &[ProviderModule],
) -> ProviderEnv {
    todo!()
}
