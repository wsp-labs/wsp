// SPDX-License-Identifier: AGPL-3.0-only
//! Port of packages/runtime/src/adapters.ts at 65f6b89c7: one adapter factory per agent wsp can drive, and what the
//! vault hands each. Spec: spec/agent_setups.md.
use std::sync::Arc;

use indexmap::IndexMap;
use serde_json::{Map, Value};
use wsp_adapters::port::{HarnessAdapter, PlanResets};
use wsp_wire::types::usage::HarnessLimit;
use wsp_words::exit::HostError;

use crate::types::harness::HarnessAdapterContext;

/// A factory that can refuse, as the TS's do by throwing (a relative config folder, a key beside a ChatGPT broker).
/// `types::harness::HarnessAdapterFactory` cannot fail; the join reconciles the two (spec, Decisions 1).
pub type AdapterFactory = Arc<dyn Fn(HarnessAdapterContext) -> Result<Arc<dyn HarnessAdapter>, HostError>>;

/// What the vault holds for one agent: its token, else its key with the variable it travels under, never both.
/// adapters.ts:37.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Secrets {
    pub oauth_token: Option<String>,
    pub api_key: Option<String>,
    pub key_env: Option<String>,
}

/// adapters.ts:33. Nothing where a login of the agent's own stands or the catalog has no such agent.
pub fn secrets_of(vault: &IndexMap<String, String>, id: &str, login_stands: bool) -> Secrets {
    todo!()
}

/// adapters.ts:50. The same under the variable the agent reads it from: one entry or none.
pub fn secret_env_of(vault: &IndexMap<String, String>, id: &str, login_stands: bool) -> IndexMap<String, String> {
    todo!()
}

/// The sign-in a person runs on a machine: the row's headless fallback, else its login; refused where the catalog
/// row has no login. adapters.ts:18.
pub fn machine_login(id: &str) -> Result<String, HostError> {
    todo!()
}

/// One factory per THREAD_AGENTS id, in that order: claude, codex, opencode, cursor. adapters.ts:57.
pub fn harness_adapters() -> IndexMap<&'static str, AdapterFactory> {
    todo!()
}

/// The agents a brokered machine launches: claude and codex. adapters.ts:82.
pub const BROKERED_AGENTS: [&str; 2] = ["claude", "codex"];

/// The agents whose plans bank resets a person can spend: codex alone. adapters.ts:85.
pub fn plan_resets() -> IndexMap<&'static str, Arc<dyn PlanResets>> {
    todo!()
}

/// A reader of one rate-limits reading off an agent's store.
pub type LogLimit = fn(&Map<String, Value>) -> Option<HarnessLimit>;

/// The agents whose own store keeps the plan's windows: codex alone, read with no account and no credits.
/// adapters.ts:89.
pub fn log_limits() -> IndexMap<&'static str, LogLimit> {
    todo!()
}
