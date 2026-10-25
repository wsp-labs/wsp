// SPDX-License-Identifier: AGPL-3.0-only
//! Port of `packages/runtime/src/threads/conversations.ts` at 65f6b89c7: the agents' own conversations outside
//! wsp. Spec: `spec/conversations.md`.
use std::cell::RefCell;
use std::rc::{Rc, Weak};
use std::sync::Arc;

use futures::future::{LocalBoxFuture, Shared};
use indexmap::{IndexMap, IndexSet};
use serde_json::{Map, Value};
use wsp_adapters::port::{ConversationLine, ConversationRoad, ConversationStore, StoredConversation};
use wsp_core::clock::Clock;
use wsp_core::context::StateCell;
use wsp_core::types::api::{ConversationAsk, OutsideOpening, Placed, PlacedStart};
use wsp_core::types::events::EventBus;
use wsp_core::types::wiring::WorkspaceRecord;
use wsp_wire::types::conversations::{ConversationRead, ConversationsAnswer, ConversationsHeld, ResumeAsk};
use wsp_wire::views::preferences::Preferences;
use wsp_wire::views::session::SessionView;
use wsp_wire::views::session_events_more::SessionEarlierEvent;
use wsp_wire::views::workspace::Caller;
use wsp_words::exit::HostError;

use super::threads::{Entry, Sessions};
use wsp_account::projects::folders::place::FolderPlaced;
use wsp_account::projects::folders::tree::ListedWorktree;
use wsp_wire::views::workspace::ProjectView;
use wsp_account::account::core_types::AccountError;
use super::transcripts::TranscriptState;

/// How long one read of an agent's store on the project's computer may take.
pub const STORE_EXEC_MS: u64 = 30_000;
/// How long a project's kept list answers a visit while its stores' stamp stays the same and no thread of it ends.
pub const KEPT_LIST_MS: i64 = 60_000;

pub use wsp_wire::types::conversations::{EARLIER_MESSAGES, PREVIEW_MESSAGES, PREVIEW_TAIL_BYTES};
/// conversations.ts:53 `StoreRead`.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct StoreRead {
    pub listed: Vec<StoredConversation>,
    pub live: Option<IndexSet<String>>,
    pub held: Option<ConversationsHeld>,
    pub passing: bool,
}

/// conversations.ts:61 `Kept`.
#[derive(Clone)]
pub struct Kept {
    pub at: i64,
    pub cwds: String,
    pub stamp: String,
    pub read: Shared<LocalBoxFuture<'static, Result<StoreRead, HostError>>>,
}

/// Every `ctx.<x>` the conversations area calls (README R2).
pub trait ConversationsCtx {
    fn clock(&self) -> Rc<dyn Clock>;
    fn bus(&self) -> Rc<dyn EventBus>;
    fn sessions(&self) -> Sessions;
    fn transcript_state(&self) -> StateCell<TranscriptState>;
    /// `ctx.live.get(ws)?.record.project`.
    fn live_project(&self, workspace_id: &str) -> Option<String>;
    fn with_daemon(&self, entry: &Entry, frame: Map<String, Value>) -> LocalBoxFuture<'static, Result<Value, AccountError>>;
    /// `harnessExec(machine, ms)` as a road's exec.
    fn harness_exec(&self, entry: &Entry, timeout_ms: u64) -> Arc<dyn wsp_adapters::port::HarnessExec>;
    fn project_held(&self, project: &str) -> ProjectView;
    fn worktrees_of(&self, top: &str) -> LocalBoxFuture<'static, Result<Vec<ListedWorktree>, HostError>>;
    /// `ctx.foldersOf(project)`: every folder record of the project.
    fn folders_of(&self, project: &str) -> Vec<WorkspaceRecord>;
    fn has_adapter(&self, harness: &str) -> bool;
    fn agent_off(&self, entry: &Entry, agent: &str) -> bool;
    /// `ctx.adapterFor(entry, harness).adapter.conversations`.
    fn conversation_store(&self, entry: &Entry, harness: &str) -> Option<Arc<dyn ConversationStore>>;
    fn agent_label(&self, agent: &str) -> String;
    fn computer_of(&self, entry: &Entry) -> String;
    fn latest_on(&self, thread_id: &str) -> Option<SessionView>;
    fn reaches_row(&self, row: &SessionView, origin: Option<&Caller>) -> bool;
    fn folder_for(&self, project: Option<String>, cwd: Option<String>, origin: Option<Caller>) -> LocalBoxFuture<'static, Result<FolderPlaced, HostError>>;
    fn entry_of(&self, workspace_id: &str, origin: Option<Caller>) -> LocalBoxFuture<'static, Result<Entry, HostError>>;
    /// `ctx.state.preferencesHeld ?? await ctx.preferences.get()`.
    fn preferences(&self) -> LocalBoxFuture<'static, Result<Preferences, HostError>>;
    fn default_agent_of(&self, prefs: &Preferences, entry: &Entry) -> String;
}

/// conversations.ts:203 `earlierRows`: V10.
pub fn earlier_rows(lines: &[ConversationLine], left: u32, label: &str) -> Vec<SessionEarlierEvent> {
    todo!()
}

/// The conversations area (`conversationsArea`, conversations.ts:68).
pub struct ConversationsArea {
    pub(crate) ctx: Rc<dyn ConversationsCtx>,
    pub(crate) me: Weak<ConversationsArea>,
    /// conversations.ts:73
    pub kept: RefCell<IndexMap<String, Kept>>,
}

impl ConversationsArea {
    /// Builds the area and registers the forgets on `session.done` and `session.end` (conversations.ts:79-80).
    pub fn new(ctx: Rc<dyn ConversationsCtx>) -> Rc<ConversationsArea> {
        todo!()
    }
    /// conversations.ts:75: V1.
    pub fn forget(&self, workspace_id: &str) {
        todo!()
    }
    /// conversations.ts:83: V2. Agent session to thread.
    pub fn wsp_sessions(&self) -> IndexMap<String, String> {
        todo!()
    }
    /// conversations.ts:90: V3.
    pub fn road_of(&self, entry: &Entry) -> ConversationRoad {
        todo!()
    }
    /// conversations.ts:97: V4.
    pub async fn folders_of(&self, entry: &Entry) -> Result<Vec<String>, HostError> {
        todo!()
    }
    /// conversations.ts:106: V5.
    pub fn stores_on(&self, entry: &Entry, agent: Option<&str>) -> Vec<(String, Arc<dyn ConversationStore>)> {
        todo!()
    }
    /// conversations.ts:114: V6.
    pub fn unread_of(&self, said: &str, entry: &Entry, harness: &str) -> Option<ConversationsHeld> {
        todo!()
    }
    /// conversations.ts:125: V7.
    pub async fn read_store(&self, entry: &Entry, cwds: &[String], road: ConversationRoad, harness: &str, store: Arc<dyn ConversationStore>) -> Result<StoreRead, HostError> {
        todo!()
    }
    /// conversations.ts:147: V8.
    pub async fn kept_read(
        &self,
        entry: &Entry,
        cwds: &[String],
        road: ConversationRoad,
        harness: &str,
        store: Arc<dyn ConversationStore>,
        mine: &IndexMap<String, String>,
    ) -> Result<StoreRead, HostError> {
        todo!()
    }
    /// conversations.ts:168: V9.
    pub async fn list(&self, project: Option<String>, agent: Option<String>, origin: Option<Caller>) -> Result<ConversationsAnswer, HostError> {
        todo!()
    }
    /// conversations.ts:213: V11.
    pub async fn read(&self, ask: ConversationAsk, origin: Option<Caller>) -> Result<ConversationRead, HostError> {
        todo!()
    }
    /// conversations.ts:228: V12.
    pub async fn opening(&self, workspace_id: &str, harness: Option<String>, resume: ResumeAsk, origin: Option<Caller>) -> Result<OutsideOpening, HostError> {
        todo!()
    }
    /// conversations.ts:264: V13.
    pub async fn placed(&self, start: PlacedStart, resume: ResumeAsk, origin: Option<Caller>) -> Result<Placed, HostError> {
        todo!()
    }
}
