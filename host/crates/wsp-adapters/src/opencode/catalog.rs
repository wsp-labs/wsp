// SPDX-License-Identifier: AGPL-3.0-only
//! What the installed OpenCode binary reports about itself. TS adapter-opencode/src/catalog.ts. Spec: spec/opencode.md
//! rules 15-17.

use indexmap::IndexMap;

use crate::port::{AgentLaunch, HarnessCatalogAnswer};

/// TS catalog.ts:11.
pub const SEP: &str = "__WSP_CATALOG_SEP__";

/// TS catalog.ts:15.
pub fn catalog_probe_command(base_env: Option<&IndexMap<String, Option<String>>>, launch: Option<&AgentLaunch>) -> String {
    todo!()
}

/// TS catalog.ts:22.
pub fn version_probe_command(base_env: Option<&IndexMap<String, Option<String>>>, launch: Option<&AgentLaunch>) -> String {
    todo!()
}

/// TS catalog.ts:28.
pub fn parse_version(stdout: &str) -> Option<String> {
    todo!()
}

/// TS catalog.ts:46. `None` where no model was printed.
pub fn parse_catalog_probe(stdout: &str) -> Option<HarnessCatalogAnswer> {
    todo!()
}
