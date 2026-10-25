// SPDX-License-Identifier: AGPL-3.0-only
//! Claude Code's launch: its environment, its sign-in order and its command line. TS:
//! packages/adapter-claude/src/landmines.ts at a3b870a6e. Environments are [`Env`]s whose key order is the TS
//! object's: a key set again keeps its first place, a key deleted and set again goes last.

use indexmap::IndexMap;
use serde_json::{Map, Value};

use crate::pending::McpServerSpec;
use crate::port::{AdapterError, AgentLaunch, Env, ExecSecret, ServerValues, TurnImage};

/// A login environment as the runtime hands it: a `None` value is a variable named and unset (TS `undefined`).
pub type BaseEnv = IndexMap<String, Option<String>>;

/// landmines.ts:12: `^CLAUDE_CODE_`, `^CLAUDECODE$`, `^FORCE_CODE_TERMINAL$`. Whether a variable is a nesting mark.
pub fn is_landmine(key: &str) -> bool {
    todo!()
}

/// landmines.ts:41.
pub const FIRST_BYTE_ENV: &str = "CLAUDE_STREAM_FIRST_BYTE_TIMEOUT_MS";
/// landmines.ts:47.
pub const PROJECT_DIR_ENV: &str = "CLAUDE_CODE_PROJECT_DIR_NAME";
/// landmines.ts:159.
pub const KEY_ENV: &str = "ANTHROPIC_API_KEY";

/// landmines.ts:53.
pub fn terminal_resume_command(base: Option<&BaseEnv>, launch: Option<&AgentLaunch>) -> String {
    todo!()
}

/// The kind of credential the host adds at the door. TS `BrokerStatus["claude"]`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BrokerKind {
    Login,
    Token,
    Key,
    Gateway,
}

/// landmines.ts:78.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BrokeredSignIn {
    pub token: String,
    pub kind: BrokerKind,
}

/// landmines.ts:58.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ClaudeEnvOptions {
    pub base: Option<BaseEnv>,
    pub api_key: Option<String>,
    pub oauth_token: Option<String>,
    pub project_dir_name: Option<String>,
    pub brokered: Option<BrokeredSignIn>,
}

/// landmines.ts:84: `http://127.0.0.1:<BROKER_PORT>`.
pub fn broker_door_url() -> String {
    todo!()
}

/// landmines.ts:104, keys in that order.
pub fn brokered_sign_in_env(b: &BrokeredSignIn) -> Env {
    todo!()
}

/// landmines.ts:116. Drops unset values and nesting marks, keeping PLUGIN_SEED_ENV.
pub fn strip_landmine_env(base: &BaseEnv) -> Env {
    todo!()
}

/// landmines.ts:137.
pub fn build_env(options: &ClaudeEnvOptions) -> Env {
    todo!()
}

/// landmines.ts:180. `""` where the vault signs in or nothing was inherited.
pub fn sign_in_guard(options: &ClaudeEnvOptions, launch: Option<&AgentLaunch>) -> String {
    todo!()
}

/// landmines.ts:191. A random v4 UUID, lower case.
pub fn new_session_id() -> String {
    todo!()
}

/// landmines.ts:195, case-insensitive.
pub fn is_uuid(id: &str) -> bool {
    todo!()
}

/// landmines.ts:197.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct BuildCommandOptions {
    pub session_id: Option<String>,
    pub resume: Option<String>,
    pub copy_of: Option<String>,
    pub cwd: Option<String>,
    pub model: Option<String>,
    pub effort: Option<String>,
    pub permission_mode: Option<String>,
    pub context_window: Option<String>,
    pub name: Option<String>,
    pub mcp_servers: Option<IndexMap<String, McpServerSpec>>,
    pub server_values: bool,
    pub off_servers: Option<Vec<String>>,
    pub fast: Option<bool>,
    pub memory_dir: Option<String>,
    pub launch: Option<AgentLaunch>,
    pub subagent_text: bool,
    pub aside: bool,
}

/// landmines.ts:243.
pub fn memory_settings(dir: Option<&str>) -> Map<String, Value> {
    todo!()
}

/// landmines.ts:252.
pub const OFF_SERVERS_FILE: &str = "/etc/claude-code/managed-settings.d/wsp-servers-off.json";

/// landmines.ts:260.
pub fn settings_flag(settings: &Map<String, Value>) -> Vec<String> {
    todo!()
}

/// The person's launch words less a JSON --settings, and that object. landmines.ts:266.
#[derive(Clone, Debug, PartialEq)]
pub struct PersonSettings {
    pub words: Vec<String>,
    pub settings: Map<String, Value>,
    pub file: bool,
}

/// landmines.ts:266.
pub fn person_settings(args: &[String]) -> PersonSettings {
    todo!()
}

/// landmines.ts:303. True from 2.1.270 on; false where no version reads.
pub fn forwards_subagent_text(version: Option<&str>) -> bool {
    todo!()
}

/// landmines.ts:313. `Err("<name> must be a plain slug, got \"<value>\"")`.
pub fn slug_flag(flag: &str, name: &str, value: Option<&str>) -> Result<Vec<String>, AdapterError> {
    todo!()
}

/// landmines.ts:349.
pub const SERVER_VALUES_ENV: &str = "WSP_MCP_VALUES";

/// landmines.ts:353: one file under SERVER_VALUES_ENV holding `{"mcpServers": entries}`; `None` with no entries.
pub fn server_values_file(values: Option<&ServerValues>) -> Option<ExecSecret> {
    todo!()
}

/// landmines.ts:362.
pub fn mcp_config_flag(servers: Option<&IndexMap<String, McpServerSpec>>) -> Vec<String> {
    todo!()
}

/// landmines.ts:379.
pub fn build_command(options: &BuildCommandOptions) -> Result<String, AdapterError> {
    todo!()
}

/// What launch_command takes: BuildCommandOptions less the session id, the name and the two flags it sets itself.
/// landmines.ts:424.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct LaunchOptions {
    pub resume: Option<String>,
    pub copy_of: Option<String>,
    pub cwd: Option<String>,
    pub model: Option<String>,
    pub effort: Option<String>,
    pub permission_mode: Option<String>,
    pub context_window: Option<String>,
    pub mcp_servers: Option<IndexMap<String, McpServerSpec>>,
    pub off_servers: Option<Vec<String>>,
    pub fast: Option<bool>,
    pub memory_dir: Option<String>,
    pub launch: Option<AgentLaunch>,
    pub title: Option<String>,
    pub version: Option<String>,
}

/// landmines.ts:424.
pub fn launch_command(o: &LaunchOptions, local_id: &str, valued: bool) -> Result<String, AdapterError> {
    todo!()
}

/// landmines.ts:437.
pub const SAVED_SPEND_TAIL_BYTES: u64 = 8 * 1024 * 1024;

/// landmines.ts:441.
pub fn saved_spend_command(config_dir: &str, session_id: &str) -> String {
    todo!()
}

/// landmines.ts:451.
pub fn user_message_line(text: &str, session_id: &str, images: &[TurnImage], uuid: Option<&str>) -> String {
    todo!()
}
