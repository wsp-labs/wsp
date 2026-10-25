// SPDX-License-Identifier: AGPL-3.0-only
//! Claude Code's own conversations on a computer. TS: packages/adapter-claude/src/conversations.ts at a3b870a6e.

use indexmap::IndexSet;

use crate::claude::landmines::BaseEnv;
use crate::port::{AgentLaunch, ConversationOrigin, ConversationStore};

/// conversations.ts:24.
pub const PROGRAMMATIC_ENTRYPOINTS: [&str; 3] = ["sdk-cli", "sdk-ts", "sdk-py"];

/// conversations.ts:27.
pub fn origin_of(entrypoint: Option<&str>) -> ConversationOrigin {
    todo!()
}

/// conversations.ts:31. Each cwd's key, then `pinned`, each once, first place kept.
pub fn keys_for(cwds: &[String], pinned: Option<&str>) -> Vec<String> {
    todo!()
}

/// conversations.ts:34.
pub fn present_command(root: &str, keys: &[String], file: Option<&str>) -> String {
    todo!()
}

/// conversations.ts:38. Runs of at most TRANSCRIPT_DIRS_MAX (64); an empty list gives no runs.
pub fn runs_of<T: Clone>(list: &[T]) -> Vec<Vec<T>> {
    todo!()
}

/// conversations.ts:49.
pub fn live_command(launch: Option<&AgentLaunch>) -> String {
    todo!()
}

/// conversations.ts:55. `None` where stdout is not a JSON array.
pub fn parse_live(stdout: &str) -> Option<IndexSet<String>> {
    todo!()
}

/// What claude_conversations takes. conversations.ts:71.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ConversationDeps {
    pub config_dir: String,
    pub project_dir_name: Option<String>,
    pub launch: Option<AgentLaunch>,
    pub base_env: Option<BaseEnv>,
}

/// conversations.ts:71. Answers a store with list, stamp, live and earlier, and no lets_go.
pub fn claude_conversations(deps: ConversationDeps) -> std::sync::Arc<dyn ConversationStore> {
    todo!()
}
