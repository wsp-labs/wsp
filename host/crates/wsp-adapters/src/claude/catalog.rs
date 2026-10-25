// SPDX-License-Identifier: AGPL-3.0-only
//! What the installed binary reports about itself. TS: packages/adapter-claude/src/catalog.ts at a3b870a6e.

use crate::port::{AgentLaunch, HarnessCatalogProbe, ScreenCommand};

/// catalog.ts:20, in that order.
pub fn claude_screen_commands() -> Vec<ScreenCommand> {
    todo!()
}

/// catalog.ts:29.
pub const CONTEXT_WINDOWS: [&str; 2] = ["200k", "1m"];

/// catalog.ts:45.
pub fn catalog_probe_command(launch: Option<&AgentLaunch>) -> String {
    todo!()
}

/// catalog.ts:53.
pub fn version_probe_command(launch: Option<&AgentLaunch>) -> String {
    todo!()
}

/// catalog.ts:59: the first `\d+\.\d+\.\d+`.
pub fn parse_version(stdout: &str) -> Option<String> {
    todo!()
}

/// catalog.ts:143. `None` without an initialize response naming a model.
pub fn parse_catalog_probe(stdout: &str) -> Option<HarnessCatalogProbe> {
    todo!()
}
