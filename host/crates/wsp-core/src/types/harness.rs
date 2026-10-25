// SPDX-License-Identifier: AGPL-3.0-only
//! Port of packages/runtime/src/types/harness.ts. The adapter port itself (HarnessAdapter, HarnessSession, the start
//! and warm options, KeptAgent) was ported with the adapters in wsp-adapters and is re-exported here, never defined
//! twice. What stays here: the context a factory is called with, the login environments, and the picks a start
//! names. Spec: spec/harness.md.

use std::sync::Arc;

use wsp_adapters::codex::command::CodexBroker;
use wsp_adapters::port::{AgentLaunch, Env, ExecStreamFactory};
use wsp_engine::machine::Machine;
use wsp_wire::types::thread_defaults::AccessChoice;
use wsp_words::exit::HostError;

pub use wsp_adapters::port::{HarnessAdapter, HarnessSession, HarnessStartOptions, HarnessWarmOptions, KeptAgent};

/// The kind of credential the host adds for Claude Code on a brokered workspace. harness.ts:61.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BrokeredKind {
    Token,
    Key,
    Gateway,
}

/// harness.ts:61.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Brokered {
    pub token: String,
    pub kind: BrokeredKind,
}

/// What a harness factory is called with, per session start, with the workspace's current machine. harness.ts:32-73.
pub struct HarnessAdapterContext {
    pub machine: Arc<dyn Machine>,
    pub workspace_id: String,
    pub exec_stream: Arc<dyn ExecStreamFactory>,
    /// Where the harness keeps its sessions on this machine, by agent id.
    pub home: Arc<dyn Fn(&str) -> String>,
    /// The machine's login environment, exported under the harness's own on every launch.
    pub env: Env,
    pub project_key: Option<String>,
    pub sign_in_refusal: String,
    /// The vault's tokens and keys by variable; empty where the host wired no vault.
    pub vault: Env,
    pub brokered: Option<Brokered>,
    pub codex_broker: Option<CodexBroker>,
    /// Whether a login of this agent's own stands where the workspace runs.
    pub login_stands: Arc<dyn Fn(&str) -> bool>,
    pub launch: Option<AgentLaunch>,
}

/// Called per session start with the workspace's current machine (it can change on wake or upgrade). harness.ts:288.
/// It can refuse, as the TS's throw (a relative config folder, a key beside a ChatGPT broker).
pub type HarnessAdapterFactory = Arc<dyn Fn(HarnessAdapterContext) -> Result<Arc<dyn HarnessAdapter>, HostError>>;

/// What a start names of its agent and picks, read against that agent's lists before a folder is made for it.
/// harness.ts:290.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct StartPicksAsked {
    pub harness: Option<String>,
    pub model: Option<String>,
    pub effort: Option<String>,
    pub access: Option<AccessChoice>,
    pub permission_mode: Option<String>,
    pub fast: Option<bool>,
}

/// `{ IS_SANDBOX: "1", DISABLE_AUTOUPDATER: "1" }`, in that key order. harness.ts:78.
pub fn machine_sandbox_env() -> Env {
    todo!()
}

/// `{ ...GUEST_USER_ENV, PATH: TOOLS_PATH, ...MACHINE_SANDBOX_ENV }`: HOME, USER, PATH, IS_SANDBOX,
/// DISABLE_AUTOUPDATER in that order. harness.ts:82.
pub fn guest_login_env() -> Env {
    todo!()
}

/// `{ ...GUEST_USER_ENV, PATH: PLACE_WORKSPACE_PATH, ...MACHINE_SANDBOX_ENV, ...installEnv(installHomes(TOOL_PREFIX)) }`,
/// keys in that order, a later spread overwriting an earlier key in its place. harness.ts:89-94.
pub fn place_login_env() -> Env {
    todo!()
}

/// The guest's environment where `place` is None, a joined computer's otherwise; with `npm_bin`, the same map with
/// PATH replaced in its place by `on_npm_bin(npm_bin, PATH)`. harness.ts:98-101.
pub fn login_env_on(place: Option<&str>, npm_bin: Option<&str>) -> Env {
    todo!()
}
