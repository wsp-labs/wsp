// SPDX-License-Identifier: AGPL-3.0-only
//! Stand-ins for the protocol's thread-defaults.ts helpers that agent_setup and harness_catalog read and no Rust crate
//! holds yet (packages/protocol/src/thread-defaults.ts at 65f6b89c7; wsp-wire ported only its shapes). Each moves to
//! wsp-words when that crate's fix round ports thread-defaults.ts, and this file then re-exports it. Not a runtime
//! TS file. Spec: spec/agent_setups.md, "Protocol stand-ins".
use wsp_wire::types::thread_defaults::{AgentSetup, AgentSetupSet, ResolvedFolder};
use wsp_wire::views::harness::HarnessCatalog;
use wsp_words::exit::HostError;
use wsp_words::thread_defaults as words;

/// thread-defaults.ts:101. The kept setup with the change over it; `None` where every field is back on the agent's own.
pub fn setup_patched(kept: Option<&AgentSetup>, set: &AgentSetupSet) -> Option<AgentSetup> {
    words::setup_patched(kept, set)
}

/// thread-defaults.ts:402. Why a variable cannot be part of an agent's setup, or `None` where it may. `dropped` is the
/// agent's own launch's strip test (the TS's `RegExp[]`); `None` drops nothing.
pub fn agent_env_refusal(name: &str, agent: &str, dropped: Option<&dyn Fn(&str) -> bool>) -> Option<String> {
    words::agent_env_refusal(name, agent, &[]).or_else(|| dropped.filter(|dropped| dropped(name)).map(|_| words::dropped_env_line(agent, name)))
}

/// thread-defaults.ts:421.
pub fn config_dir_refusal(agent: &str, given: &str, real: &ResolvedFolder) -> Option<String> {
    words::config_dir_refusal(agent, given, real)
}

/// thread-defaults.ts:380.
pub fn no_config_dir_line(agent: &str) -> String {
    words::no_config_dir_line(agent)
}

/// thread-defaults.ts:337. A usage refusal with its fix, or `None`.
pub fn not_run_refusal(lists: Option<&HarnessCatalog>, table: Option<&HarnessCatalog>, model: Option<&str>, update: Option<&str>) -> Option<HostError> {
    words::not_run_refusal(lists, table, model, update)
}
