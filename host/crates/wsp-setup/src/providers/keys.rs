// SPDX-License-Identifier: AGPL-3.0-only
//! Port of `cli/keys.ts`: where a provider key is read from, asked for and saved, and what a run reads its keys as.
use std::rc::Rc;

use futures::future::LocalBoxFuture;
use indexmap::IndexMap;
use serde_json::Value;
use wsp_catalog::signin_rows::{server_values_of, VAULT_VARIABLES};
use wsp_cli::verbs::client::CliIo;
use wsp_engine::key_check::KeyCheck;
use wsp_engine::run_env::GITHUB_TOKEN_ENV;
use wsp_words::exit::HostError;

use crate::files::env_keys::{env_file_for, parse_env_file, server_env_file_for};
use crate::providers::providers::ProviderEnv;

/// `KeySources` (`cli/keys.ts:11`).
#[derive(Clone)]
pub struct KeySources {
    pub env: ProviderEnv,
    pub cwd: String,
    pub state_path: String,
    pub check_key: Option<Rc<dyn Fn(&str) -> LocalBoxFuture<'static, KeyCheck>>>,
    pub login: Option<Rc<dyn Fn() -> LocalBoxFuture<'static, IndexMap<String, String>>>>,
}

/// `keyLayers` (`cli/keys.ts:29`).
pub fn key_layers(_sources: &KeySources) -> Vec<ProviderEnv> {
    todo!()
}

/// `keySources` (`cli/keys.ts:36`).
pub fn key_sources(_env: &ProviderEnv, _state_path: &str) -> KeySources {
    todo!()
}

/// `providerEnvNow` (`cli/keys.ts:44`).
pub fn provider_env_now(_env: &ProviderEnv, _state_path: &str, _cwd: &str) -> ProviderEnv {
    todo!()
}

/// `NoProviderKey` (`cli/keys.ts:56`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NoProviderKey {
    Refuse,
    Offer,
    Local,
}

/// `LoadedKeys` (`cli/keys.ts:60`).
#[derive(Clone, Debug, PartialEq)]
pub struct LoadedKeys {
    pub keys: Value, // Keys: env-keys.ts's type, no Rust form yet
    pub env: ProviderEnv,
}

/// `KeyAsk` (`cli/keys.ts:65`).
#[derive(Clone, Debug, PartialEq)]
pub struct KeyAsk {
    pub anthropic: bool,
    pub no_provider_key: Option<NoProviderKey>,
    pub check_saved: Option<bool>,
    pub provider: Option<String>,
    pub save: Option<bool>,
}

/// `loadKeys` (`cli/keys.ts:76`).
pub async fn load_keys(_io: &dyn CliIo, _sources: &KeySources, _ask: &KeyAsk) -> Result<LoadedKeys, HostError> {
    todo!()
}

/// `keysFound` (`cli/keys.ts:153`).
pub fn keys_found(_sources: &KeySources, _layers: &[ProviderEnv]) -> Value {
    todo!()
}

/// `vaultNow` (`cli/keys.ts:165`): what the one file holds under a catalog row's variable or the GitHub token, and the servers' values
/// beside it; read at each ask, nothing cached.
pub fn vault_now(state_path: &str) -> IndexMap<String, String> {
    let saved = parse_env_file(&env_file_for(state_path));
    let mut vault: IndexMap<String, String> = VAULT_VARIABLES
        .iter()
        .map(String::as_str)
        .chain([GITHUB_TOKEN_ENV])
        .filter_map(|name| saved.get(name).map(|value| (name.to_string(), value.clone())))
        .collect();
    vault.extend(server_values_of(&parse_env_file(&server_env_file_for(state_path))));
    vault
}

/// `saveQuestion` (`cli/keys.ts:170`).
pub fn save_question(_folder: &str, _keys: usize) -> String {
    todo!()
}
