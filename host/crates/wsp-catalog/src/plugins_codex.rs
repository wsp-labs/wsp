// SPDX-License-Identifier: AGPL-3.0-only
//! Codex's plugins, asked of `codex app-server`, and the script that asks it. Port of
//! packages/catalog/src/plugins-codex.ts and codex-app-server.ts at a3b870a6e; the spec is spec/plugins_codex.md.
use indexmap::IndexMap;
use serde_json::{Map, Value};

use crate::plugins::{FoundPlugin, PluginIo, PluginsFound};

/// codex-app-server.ts:448.
pub const LINE_WAIT_S: u64 = 10;

/// `{"jsonrpc":"2.0","id":<id>,"method":<method>,"params":<params>}` as `JSON.stringify` writes it
/// (codex-app-server.ts:450).
pub fn request(id: i64, method: &str, params: Value) -> String {
    todo!()
}

/// codex-app-server.ts:452.
pub fn notification(method: &str) -> String {
    todo!()
}

/// codex-app-server.ts:454.
pub fn initialize_request(id: i64) -> String {
    todo!()
}

/// codex-app-server.ts:457.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServerStage {
    pub lines: Vec<String>,
    pub answers: u64,
}

/// codex-app-server.ts:463.
pub fn app_server_script(codex: &str, stages: &[ServerStage]) -> String {
    todo!()
}

/// codex-app-server.ts:493.
#[derive(Debug, Clone, PartialEq)]
pub enum ServerAnswer {
    Result(Map<String, Value>),
    Error { code: Option<i64>, message: String, data: Option<Map<String, Value>> },
}

/// Each answer by its numeric id, the last of an id winning (codex-app-server.ts:500).
pub fn answers_of(text: &str) -> IndexMap<i64, ServerAnswer> {
    todo!()
}

/// codex-app-server.ts:525.
pub fn result_of(answers: &IndexMap<i64, ServerAnswer>, id: i64) -> Option<&Map<String, Value>> {
    todo!()
}

/// plugins-codex.ts:344.
pub async fn read_codex_plugins(io: &impl PluginIo) -> PluginsFound {
    todo!()
}

/// plugins-codex.ts:421.
pub async fn turn_codex(io: &impl PluginIo, plugin: &FoundPlugin, on: bool, read: &PluginsFound) -> Option<String> {
    todo!()
}
