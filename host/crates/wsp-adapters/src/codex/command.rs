// SPDX-License-Identifier: AGPL-3.0-only
//! The bash line that runs `codex app-server` for one turn, the environment it runs under, and the launch words every
//! other codex command starts with; on a brokered machine, the placeholder login and the door. TS
//! adapter-codex/src/command.ts. Spec: spec/codex_command.md. `now_ms` stands for the TS `Date.now()`.

use std::sync::Arc;

use futures::future::BoxFuture;
use indexmap::IndexMap;
use serde_json::{Map, Value};

use crate::pending::{CodexClaims, McpServerSpec};
use crate::port::{AgentLaunch, Env};

/// The variable each custom provider's env_key names on a brokered turn. TS command.ts:22.
pub const PROVIDER_KEY_ENV: &str = "WSP_CODEX_PROVIDER_KEY";

/// Where a brokered launch finds its placeholder key until it writes the key file. TS command.ts:46.
pub const KEY_FILE_ENV: &str = "WSP_CODEX_KEY";

/// The door on the machine's loopback. TS command.ts:16.
pub const DOOR: &str = "http://127.0.0.1:7071";

/// The same door under the second origin, for chatgpt_base_url alone. TS command.ts:20.
pub const BACKEND_DOOR: &str = "http://localhost:7071";

/// The model address of a launch that asks no model. TS command.ts:221.
pub const NO_MODELS: &str = "http://127.0.0.1:0";

/// ChatGPT's list of the models the host's sign-in may run on the codex version named, read by the host: `Ok(None)`
/// or `Err` where it could not read one. TS command.ts:27.
pub type AccountModels = Arc<dyn Fn(String) -> BoxFuture<'static, Result<Option<Value>, String>> + Send + Sync>;

/// Which placeholder login a brokered machine runs on. TS command.ts:31.
#[derive(Clone)]
pub enum BrokerMode {
    Chatgpt { claims: CodexClaims, models: Option<AccountModels> },
    Apikey,
}

/// A turn on a machine whose credentials the host keeps. TS command.ts:31.
#[derive(Clone)]
pub struct CodexBroker {
    pub mode: BrokerMode,
    /// The person's own `model_providers` entries, each a JSON object in its own key order.
    pub providers: Option<IndexMap<String, Map<String, Value>>>,
}

/// The two broker modes by name, as `provider_flags` takes them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BrokerKind {
    Chatgpt,
    Apikey,
}

/// TS command.ts:35.
#[derive(Clone, Default)]
pub struct CodexEnvOptions {
    /// The base environment; an entry whose value is `None` is left out.
    pub base: Option<IndexMap<String, Option<String>>>,
    /// Absolute path for CODEX_HOME.
    pub home: String,
    pub api_key: Option<String>,
    pub broker: Option<CodexBroker>,
}

/// CODEX_HOME and the key variables over the base environment. TS command.ts:66.
pub fn build_env(options: &CodexEnvOptions) -> Result<Env, String> {
    todo!()
}

/// The placeholder key where the launch reads one, and the refresh and revoke addresses at the door. TS command.ts:76.
pub fn brokered_codex_env(broker: &CodexBroker, api_key: Option<&str>) -> Result<Env, String> {
    todo!()
}

/// One of the person's providers as overrides of the copied config's entry. TS command.ts:140.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProviderFlags {
    pub flags: Vec<String>,
    pub keyed: bool,
}

/// TS command.ts:140; `models` defaults to DOOR there.
pub fn provider_flags(id: &str, entry: &Map<String, Value>, mode: BrokerKind, models: &str) -> Result<ProviderFlags, String> {
    todo!()
}

/// What a launch that is not a turn starts codex with: the word, and the broker's overrides for after the subcommand.
/// TS command.ts:198.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CodexLaunch {
    pub word: String,
    pub flags: Vec<String>,
}

/// TS command.ts:198.
pub fn codex_launch(launch: Option<&AgentLaunch>, broker: Option<&CodexBroker>, now_ms: i64) -> Result<CodexLaunch, String> {
    todo!()
}

/// How a launch that asks no model starts codex, the overrides ahead of the subcommand. TS command.ts:210.
pub fn codex_word(launch: Option<&AgentLaunch>, broker: Option<&CodexBroker>) -> Result<String, String> {
    todo!()
}

/// The program a terminal on a brokered machine finds as codex in `folder`. TS command.ts:231.
pub fn codex_for_shell(broker: &CodexBroker, folder: &str, now_ms: i64) -> Result<String, String> {
    todo!()
}

/// TS command.ts:242.
#[derive(Clone, Default)]
pub struct BuildCommandOptions {
    pub cwd: Option<String>,
    pub mcp_servers: Option<IndexMap<String, McpServerSpec>>,
    pub launch: Option<AgentLaunch>,
    pub broker: Option<CodexBroker>,
    pub off_servers: Option<Vec<String>>,
    /// `true` is the TS `turn: false`: a server run that starts no turn, on no login. The default is a turn's launch.
    pub side_run: bool,
}

/// A value that may ride a codex command line or a SQL literal unquoted. TS command.ts:257.
pub fn slug(name: &str, value: &str) -> Result<String, String> {
    todo!()
}

/// TS command.ts:310.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Sandbox {
    ReadOnly,
    WorkspaceWrite,
    DangerFullAccess,
}

/// TS command.ts:312.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ApprovalPolicy {
    OnRequest,
    Never,
}

/// The sandbox and approval pair a thread's start carries, written `sandbox` then `approvalPolicy`. TS command.ts:310.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AccessParams {
    pub sandbox: Sandbox,
    pub approval_policy: ApprovalPolicy,
}

impl Sandbox {
    /// `read-only`, `workspace-write`, `danger-full-access`.
    pub fn word(&self) -> &'static str {
        todo!()
    }
}

impl ApprovalPolicy {
    /// `on-request`, `never`.
    pub fn word(&self) -> &'static str {
        todo!()
    }
}

/// TS command.ts:317.
pub fn access_params(mode: Option<&str>) -> Result<AccessParams, String> {
    todo!()
}

/// The server as one bash line in the folder the turn runs in. TS command.ts:326.
pub fn build_command(options: &BuildCommandOptions, now_ms: i64) -> Result<String, String> {
    todo!()
}

/// TS command.ts:334.
pub fn image_path(path: &str) -> Result<String, String> {
    todo!()
}
