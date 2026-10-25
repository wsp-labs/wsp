// SPDX-License-Identifier: AGPL-3.0-only
//! Codex's own threads on a computer, read through `codex app-server` there. TS adapter-codex/src/conversations.ts and
//! the store at adapter.ts:1331-1388. Spec: spec/codex_conversations.md.

use std::sync::Arc;

use futures::future::LocalBoxFuture;
use serde_json::{Map, Value};

use super::command::CodexBroker;
use crate::port::{AgentLaunch, ConversationEarlier, ConversationLine, ConversationRoad, ConversationStore, EarlierAsk, EarlierRead, Env, StoreError, StoredConversation};

/// Codex's words for a thread another process writes. TS conversations.ts:16.
pub const WRITER_HELD: &str = "already has an active writer";

/// TS conversations.ts:21.
pub const LETS_GO: &str = "Codex lets go about a minute after its window closes.";

/// How many pages a list or a read takes at most. TS conversations.ts:26.
pub const LIST_PAGES: usize = 10;

/// What a list prints in place of the server's answers where codex is not on the PATH. TS conversations.ts:41.
pub const NOT_INSTALLED: &str = "wsp-codex-not-installed";

/// Who a script runs codex as. TS conversations.ts:45's `road`.
#[derive(Clone, Default)]
pub struct CodexRoad {
    pub launch: Option<AgentLaunch>,
    pub broker: Option<CodexBroker>,
}

/// A thread as the list shows it, with the rollout path the size is read from. TS conversations.ts:63.
#[derive(Clone, Debug, PartialEq)]
pub struct ListedRow {
    pub row: StoredConversation,
    pub path: Option<String>,
}

/// One page of thread/list. TS conversations.ts:91.
#[derive(Clone, Debug, PartialEq)]
pub struct ListPage {
    pub rows: Vec<ListedRow>,
    pub next: Option<String>,
}

/// TS conversations.ts:54.
pub fn list_command(cwds: &[String], cursor: Option<&str>, road: &CodexRoad) -> Result<String, String> {
    todo!()
}

/// TS conversations.ts:91.
pub fn parse_list(stdout: &str) -> Result<ListPage, StoreError> {
    todo!()
}

/// TS conversations.ts:101.
pub fn newest_day_stamp_command(home: &str) -> String {
    todo!()
}

/// TS conversations.ts:108.
pub fn sizes_command(paths: &[String]) -> String {
    todo!()
}

/// TS conversations.ts:112. Always `count` entries.
pub fn parse_sizes(stdout: &str, count: usize) -> Vec<Option<f64>> {
    todo!()
}

/// TS conversations.ts:122.
pub fn turns_command(thread_id: &str, probe: bool, cursor: Option<&str>, road: &CodexRoad) -> Result<String, String> {
    todo!()
}

/// TS conversations.ts:130.
pub fn items_command(thread_id: &str, cursor: Option<&str>, road: &CodexRoad) -> Result<String, String> {
    todo!()
}

/// What opening the thread said. TS conversations.ts:136.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Opened {
    Gone,
    Open,
    Read { cwd: Option<String>, title: Option<String> },
}

/// TS conversations.ts:136.
pub fn parse_opened(stdout: &str) -> Result<Opened, String> {
    todo!()
}

/// One turn's id and how many messages its summary holds.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TurnCount {
    pub id: String,
    pub messages: u32,
}

/// TS conversations.ts:151.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TurnsPage {
    pub turns: Vec<TurnCount>,
    pub next: Option<String>,
}

/// TS conversations.ts:151.
pub fn parse_turns(stdout: &str) -> Result<TurnsPage, String> {
    todo!()
}

/// One item's row and its turn.
#[derive(Clone, Debug, PartialEq)]
pub struct ItemLine {
    pub turn_id: Option<String>,
    pub line: ConversationLine,
}

/// TS conversations.ts:168.
#[derive(Clone, Debug, PartialEq)]
pub struct ItemsPage {
    pub items: Vec<ItemLine>,
    pub next: Option<String>,
}

/// A call as a thread's earlier messages show it, `{tool, text}`. codex_items' `tool_call_of` is the one handed in.
pub type ToolOf = fn(&Map<String, Value>) -> Option<(String, String)>;

/// TS conversations.ts:168.
pub fn parse_items(stdout: &str, tool_of: ToolOf) -> Result<ItemsPage, String> {
    todo!()
}

/// The newest `last` messages, the calls among them kept, and how many of `total` came before. TS conversations.ts:200.
pub fn newest_lines(items: &[ConversationLine], total: u32, last: u32) -> ConversationEarlier {
    todo!()
}

/// The threads Codex kept on a computer. TS adapter.ts:1333.
pub struct CodexConversations {
    pub home: String,
    pub road: CodexRoad,
    /// The environment every question carries off its input: the turn's own where brokered, else build_env's. TS
    /// adapter.ts:1331.
    pub question_env: Env,
}

impl ConversationStore for CodexConversations {
    fn list(&self, cwds: Vec<String>, road: ConversationRoad) -> LocalBoxFuture<'static, Result<Vec<StoredConversation>, StoreError>> {
        todo!()
    }
    fn stamp(&self, cwds: Vec<String>, road: ConversationRoad) -> Option<LocalBoxFuture<'static, Result<String, StoreError>>> {
        todo!()
    }
    fn earlier(&self, id: String, ask: EarlierAsk, road: ConversationRoad) -> LocalBoxFuture<'static, Result<EarlierRead, StoreError>> {
        todo!()
    }
    fn lets_go(&self) -> Option<String> {
        todo!()
    }
}

/// The store as the adapter hands it out.
pub fn codex_conversations(home: &str, road: CodexRoad, question_env: Env) -> Arc<CodexConversations> {
    todo!()
}
