// SPDX-License-Identifier: AGPL-3.0-only
//! `packages/runtime/src/machines/brokered.ts` at 65f6b89c7: what a turn and a terminal carry on a machine whose
//! credentials the host keeps. Pure. Spec: spec/brokered.md.

use std::sync::Arc;

use indexmap::IndexMap;
use wsp_adapters::claude::landmines::BrokeredSignIn;
use wsp_adapters::codex::command::{AccountModels, CodexBroker};
use wsp_adapters::pending::CodexClaims;
use wsp_adapters::port::ExecStreamFactory;
use wsp_wire::wire::runtime::BrokerStatus;
use wsp_words::exit::HostError;

/// The folder gh reads its config from on such a machine (brokered.ts:150).
pub const BROKERED_GH_DIR: &str = "/run/wsp-daemon/gh";

/// The folder of the agents' programs a terminal finds, one folder per agent (brokered.ts:158).
pub const BROKERED_BIN_DIR: &str = "/usr/local/libexec/wsp-terminal";

/// The variable only a terminal's shell carries, naming its thread's agent (brokered.ts:161).
pub const TERMINAL_ENV: &str = "WSP_TERMINAL";

/// The profile file that puts that agent's folder first (brokered.ts:166).
pub const TERMINAL_PROFILE: &str = "/etc/profile.d/wsp-terminal.sh";

/// The folder a terminal whose thread runs `own` finds the agents' programs in (brokered.ts:163). Unquoted.
pub fn terminal_bin_of(own: &str) -> String {
    todo!()
}

/// The program a terminal finds as `other`'s command where its thread runs `own` (brokered.ts:172).
pub fn other_agent_program(other: &str, own: &str) -> String {
    todo!()
}

/// The line a terminal's open runs on such a machine (brokered.ts:181), spec rules 5 and 6.
pub fn terminal_line(own: &str, own_program: Option<&str>) -> String {
    todo!()
}

/// The repo of record as `brokeredEnv` reads it: `Pick<BranchOfRecord, "owner" | "name" | "public">`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RepoOfRecord {
    pub owner: String,
    pub name: String,
    pub public: bool,
}

/// Every turn's environment on such a machine (brokered.ts:198), in the TS key order.
pub fn brokered_env(token: &str, repo: Option<&RepoOfRecord>, direct: &[String]) -> IndexMap<String, String> {
    todo!()
}

/// The kind of credential the host's broker adds for Claude Code.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ClaudePlaceholderKind {
    Token,
    Key,
    Gateway,
}

/// The sign-in a turn's agent is handed on such a machine (brokered.ts:215).
#[derive(Clone, Debug, PartialEq)]
pub struct Placeholders {
    pub claude_token: String,
    pub claude_kind: ClaudePlaceholderKind,
    pub vault: IndexMap<String, String>,
}

/// `placeholdersOf` (brokered.ts:215), spec rule 8.
pub fn placeholders_of(status: Option<&BrokerStatus>, token: &str) -> Placeholders {
    todo!()
}

/// A launch on such a machine as the host reads it for an agent (brokered.ts:222 `LaunchRead`'s answer).
#[derive(Clone)]
pub struct LaunchRead {
    pub vault: IndexMap<String, String>,
    pub claude: Option<BrokeredSignIn>,
    pub codex: Option<CodexBroker>,
}

/// What an agent signs in with in a terminal's shell (brokered.ts:228).
#[derive(Clone, Debug, PartialEq)]
pub struct TerminalSignIn {
    pub env: IndexMap<String, String>,
    pub program: Option<String>,
}

/// `terminalSignIn` (brokered.ts:243), spec rule 9. `now_ms` is the clock's, which wsp-adapters' `codex_for_shell`
/// takes; an adapter refusal is answered as `Err` with its words.
pub fn terminal_sign_in(own: &str, read: &dyn Fn(Option<&str>) -> Option<LaunchRead>, token: &str, now_ms: i64) -> Result<Option<TerminalSignIn>, String> {
    todo!()
}

/// `codexBrokerOf` (brokered.ts:249), spec rule 10.
pub fn codex_broker_of(status: Option<&BrokerStatus>, claims: Option<&CodexClaims>, models: Option<AccountModels>) -> Option<CodexBroker> {
    todo!()
}

/// `unbrokeredRefusal` (brokered.ts:255), spec rule 11: kind `usage`.
pub fn unbrokered_refusal(harness: &str) -> Option<HostError> {
    todo!()
}

/// `withGhDoor` (brokered.ts:261), spec rule 12.
pub fn with_gh_door(exec: Arc<dyn ExecStreamFactory>) -> Arc<dyn ExecStreamFactory> {
    todo!()
}
