// SPDX-License-Identifier: AGPL-3.0-only
//! `env-keys.ts`: the dotenv a host keeps beside its state file, the keys read out of it, and the one writer of that file.
use std::fs;
use std::rc::Rc;

use indexmap::IndexMap;
use wsp_words::exit::HostError;

use crate::context::Env;
use crate::paths::{dirname, join};

/// `ANTHROPIC_KEY` (`env-keys.ts:14`).
pub const ANTHROPIC_KEY: &str = "ANTHROPIC_API_KEY";

/// `KEY_LAYER_WORDS` (`env-keys.ts:19`).
pub const KEY_LAYER_WORDS: &str = "the environment, ./.env, or the .env beside your state file (~/.wsp/.env unless you named a state)";

/// `Keys` (`env-keys.ts:21`).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Keys {
    pub anthropic: Option<String>,
}

/// `parseEnvFile` (`env-keys.ts:27`): `NAME=value` lines, a line with no value dropped, the value trimmed.
pub fn parse_env_file(path: &str) -> IndexMap<String, String> {
    let mut out = IndexMap::new();
    let Ok(text) = fs::read_to_string(path) else { return out };
    for line in text.split('\n') {
        let Some((name, value)) = line.split_once('=') else { continue };
        let mut chars = name.chars();
        let named =
            chars.next().is_some_and(|c| c.is_ascii_alphabetic() || c == '_') && chars.all(|c| c.is_ascii_alphanumeric() || c == '_');
        let value = value.trim();
        if named && !value.is_empty() {
            out.insert(name.to_string(), value.to_string());
        }
    }
    out
}

pub fn env_file_for(state_path: &str) -> String {
    join(&[&dirname(state_path), ".env"])
}

pub fn server_env_file_for(state_path: &str) -> String {
    join(&[&dirname(state_path), "servers.env"])
}

/// `serverOwnersFileFor` (`env-keys.ts:49`).
pub fn server_owners_file_for(_state_path: &str) -> String {
    todo!()
}

/// `ServerVault` (`env-keys.ts:53`), what `serverVault` hands back for one state file.
pub trait ServerVault {
    /// `ServerVault.file` (`env-keys.ts:54`).
    fn file(&self) -> String;
    /// `ServerVault.held` (`env-keys.ts:56`).
    fn held(&self) -> IndexMap<String, String>;
    /// `ServerVault.owners` (`env-keys.ts:57`).
    fn owners(&self) -> IndexMap<String, Vec<String>>;
    /// `ServerVault.hold` (`env-keys.ts:59`).
    fn hold(&self, values: &IndexMap<String, String>, server: &str);
    /// `ServerVault.release` (`env-keys.ts:61`).
    fn release(&self, server: &str);
}

/// `serverVault` (`env-keys.ts:77`).
pub fn server_vault(_state_path: &str) -> Rc<dyn ServerVault> {
    todo!()
}

/// `savedEnv` (`env-keys.ts:112`).
pub fn saved_env(_state_path: &str) -> IndexMap<String, String> {
    todo!()
}

/// `keyIn` (`env-keys.ts:118`).
pub fn key_in(_env: &Env, _name: &str) -> Option<String> {
    todo!()
}

/// `keysOf` (`env-keys.ts:124`).
pub fn keys_of(_env: &Env) -> Keys {
    todo!()
}

/// `vaultOf` (`env-keys.ts:132`).
pub fn vault_of(_env: &Env) -> IndexMap<String, String> {
    todo!()
}

/// `writeEnvFile` (`env-keys.ts:139`), where a `None` value takes its line out.
pub fn write_env_file(_path: &str, _set: &IndexMap<String, Option<String>>) -> Result<(), HostError> {
    todo!()
}
