// SPDX-License-Identifier: AGPL-3.0-only
//! The servers one scope of an agent's JSON config carries in the copy that travelled. Port of
//! packages/catalog/src/mcp-travelled.ts at a3b870a6e; spec/jsonc.md.
use crate::jsonc::JsonObject;
use wsp_words::exit::HostError;

/// A laptop folder and the machine folder its servers belong to now.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FromTo {
    pub from: String,
    pub to: String,
}

/// The object under `key` at the copy's top, or for a folder in `projects[from]`, else `projects[to]`, else nothing;
/// an empty object where the key holds no object. Errors with "the copy that travelled is not a JSON object" where the
/// copy is not one, and with the parse error where it is not JSON (mcp-travelled.ts:11).
pub fn travelled_servers(travelled: &str, key: &str, project: Option<&FromTo>) -> Result<JsonObject, HostError> {
    todo!()
}
