// SPDX-License-Identifier: AGPL-3.0-only
//! The dotenv a host keeps beside its state file and what a turn's vault reads out of it (`env-keys.ts`, `cli/keys.ts`).
use std::fs;

use indexmap::IndexMap;
use wsp_catalog::signin_rows::{server_values_of, VAULT_VARIABLES};
use wsp_engine::run_env::GITHUB_TOKEN_ENV;

use crate::paths::{dirname, join};

/// `parseEnvFile`: `NAME=value` lines, a line with no value dropped, the value trimmed.
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

/// `vaultNow`: what the one file holds under a catalog row's variable or the GitHub token, and the servers' values
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
