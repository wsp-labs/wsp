// SPDX-License-Identifier: AGPL-3.0-only
//! What the installed codex reports about itself, without a turn. TS adapter-codex/src/catalog.ts. Spec:
//! spec/codex_catalog.md.

use serde_json::{Map, Value};

use super::command::{AccountModels, CodexBroker};
use crate::port::{AgentLaunch, HarnessCatalogAnswer, HarnessCatalogModelProbe};

/// TS catalog.ts:354.
pub const SEP: &str = "__WSP_CATALOG_SEP__";

/// TS catalog.ts:373.
pub fn catalog_probe_command(launch: Option<&AgentLaunch>, broker: Option<&CodexBroker>) -> Result<String, String> {
    todo!()
}

/// TS catalog.ts:382.
pub fn version_probe_command(launch: Option<&AgentLaunch>, broker: Option<&CodexBroker>) -> Result<String, String> {
    todo!()
}

/// TS catalog.ts:387.
pub fn parse_version(stdout: &str) -> Option<String> {
    todo!()
}

/// ChatGPT's /codex/models answer as model/list rows; `None` where it shows no model. TS catalog.ts:445 (private).
pub fn account_rows(answer: Option<&Value>) -> Option<Map<String, Value>> {
    todo!()
}

/// `None` when the app server said nothing at all. TS catalog.ts:476.
pub fn parse_catalog_probe(stdout: &str, login: &str, account_list: Option<&Value>) -> Option<HarnessCatalogAnswer> {
    todo!()
}

/// TS catalog.ts:500.
pub async fn read_catalog_probe(stdout: &str, login: &str, models: Option<&AccountModels>) -> Option<HarnessCatalogAnswer> {
    todo!()
}

/// The account's models where no binary has described itself, for `version`, else wsp's npm pin. TS catalog.ts:509.
pub async fn account_models_of(models: &AccountModels, version: Option<&str>) -> Option<Vec<HarnessCatalogModelProbe>> {
    todo!()
}
