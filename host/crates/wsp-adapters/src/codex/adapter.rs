// SPDX-License-Identifier: AGPL-3.0-only
//! The Codex adapter: its launches, side questions, side runs, kept servers, probes and store. TS
//! adapter-codex/src/adapter.ts:104-249 (the types) and 513-519, 1229-1454 (`createCodexAdapter` past `follow`).
//! Spec: spec/codex_adapter.md.

use std::sync::Arc;

use futures::future::LocalBoxFuture;
use indexmap::IndexMap;

use super::command::CodexBroker;
use super::conversations::CodexConversations;
use super::turn::{CodexSession, Sessions};
use crate::port::{AdapterAttachOptions, AdapterError, AgentLaunch, AsideAnswer, AsideQuestion, DraftAsk, Env, ExecFailure, ExecStreamFactory, ForkTurn, HarnessCatalogAnswer, HarnessCatalogModelProbe, HarnessExec, HarnessStartOptions, Now, RevertAsk, Reverted, SessionFile, SessionRenameWrite, TitleTurn};

/// TS adapter.ts:172.
#[derive(Clone)]
pub struct CodexAdapterDeps {
    pub exec: Arc<dyn ExecStreamFactory>,
    /// CODEX_HOME on the machine.
    pub home: String,
    /// The catalog's command for signing codex in on a machine.
    pub login: String,
    pub api_key: Option<String>,
    /// The variable the key travels under; absent where no key was handed.
    pub key_env: Option<String>,
    pub base_env: Option<IndexMap<String, Option<String>>>,
    /// INTERRUPT_GRACE_MS (5000) where `None`.
    pub interrupt_grace_ms: Option<u64>,
    /// RUN_EXIT_MS (10000) where `None`.
    pub result_exit_ms: Option<u64>,
    /// STEER_JOIN_MS (10000) where `None`.
    pub steer_join_ms: Option<u64>,
    /// 90000 where `None`.
    pub reconnect_stall_ms: Option<u64>,
    /// ASIDE_WALL_MS (180000) where `None`.
    pub aside_wall_ms: Option<u64>,
    pub launch: Option<AgentLaunch>,
    pub broker: Option<CodexBroker>,
    /// The system clock where `None`.
    pub now: Option<Now>,
}

impl CodexAdapterDeps {
    /// Deps with every optional field `None`.
    pub fn new(exec: Arc<dyn ExecStreamFactory>, home: impl Into<String>, login: impl Into<String>) -> Self {
        CodexAdapterDeps { exec, home: home.into(), login: login.into(), api_key: None, key_env: None, base_env: None, interrupt_grace_ms: None, result_exit_ms: None, steer_join_ms: None, reconnect_stall_ms: None, aside_wall_ms: None, launch: None, broker: None, now: None }
    }
}

/// What attach found. TS adapter.ts:206 (`CodexSession | "gone"`).
pub enum AttachedSession {
    Session(CodexSession),
    Gone,
}

/// TS adapter.ts:201. Fields are the implementer's to extend.
#[derive(Clone)]
pub struct CodexAdapter {
    pub deps: CodexAdapterDeps,
    /// What every turn's process is launched with: build_env of the deps.
    pub env: Env,
    pub sessions: Sessions,
}

/// TS adapter.ts:513. `Err` where build_env refuses (a relative home, a key beside a ChatGPT broker, a key that is
/// not one plain word).
pub fn create_codex_adapter(deps: CodexAdapterDeps) -> Result<CodexAdapter, AdapterError> {
    todo!()
}

impl CodexAdapter {
    /// The capability flags of adapter.ts:1392-1427, each `true` there.
    pub fn steers(&self) -> bool {
        todo!()
    }
    pub fn reports_edits(&self) -> bool {
        todo!()
    }
    pub fn mcp_servers(&self) -> bool {
        todo!()
    }
    pub fn denies_servers(&self) -> bool {
        todo!()
    }
    pub fn waits_for_prompt(&self) -> bool {
        todo!()
    }
    pub fn forks_by_count(&self) -> bool {
        todo!()
    }
    /// `file`: each image lands on the machine first. TS adapter.ts:1394.
    pub fn attachments(&self) -> Option<crate::port::AttachmentRoad> {
        todo!()
    }
    /// TS adapter.ts:1298 (`start` through `startSeeded` and `launchTurn`). `Err` where the TS throws before anything
    /// is launched.
    pub fn start(&self, options: HarnessStartOptions) -> Result<CodexSession, AdapterError> {
        todo!()
    }
    /// TS adapter.ts:1397. `None` where the exec factory has no attach.
    pub fn attach(&self, options: AdapterAttachOptions) -> Option<LocalBoxFuture<'static, Result<AttachedSession, ExecFailure>>> {
        todo!()
    }
    /// A snapshot of the sessions map, in registration order.
    pub fn sessions(&self) -> IndexMap<String, CodexSession> {
        todo!()
    }
    /// The turn's environment. TS adapter.ts:1452.
    pub fn env(&self) -> Env {
        todo!()
    }
    /// `/compact`. TS adapter.ts:1418.
    pub fn compacts(&self) -> &'static str {
        todo!()
    }
    /// TS adapter.ts:1303. `Err` with the words the TS rejects with.
    pub async fn aside(&self, question: AsideQuestion) -> Result<AsideAnswer, String> {
        todo!()
    }
    /// TS adapter.ts:1324.
    pub async fn revert(&self, ask: RevertAsk) -> Result<Reverted, String> {
        todo!()
    }
    /// TS adapter.ts:1325. The session and turn the fork's start runs on.
    pub async fn fork_session(&self, session: &str, turn: ForkTurn) -> Result<(String, String), String> {
        todo!()
    }
    /// TS adapter.ts:1390.
    pub async fn probe_catalog(&self, exec: Arc<dyn HarnessExec>) -> Result<Option<HarnessCatalogAnswer>, ExecFailure> {
        todo!()
    }
    /// TS adapter.ts:1429.
    pub async fn probe_version(&self, exec: Arc<dyn HarnessExec>) -> Result<Option<String>, ExecFailure> {
        todo!()
    }
    /// `None` at the outer level where the host holds no ChatGPT sign-in for the machine. TS adapter.ts:1428.
    pub fn account_models(&self, version: Option<String>) -> Option<LocalBoxFuture<'static, Option<Vec<HarnessCatalogModelProbe>>>> {
        todo!()
    }
    /// TS adapter.ts:1430.
    pub async fn session_title(&self, thread_id: &str, exec: Arc<dyn HarnessExec>) -> Result<Option<String>, ExecFailure> {
        todo!()
    }
    /// TS adapter.ts:1431.
    pub fn session_file(&self, thread_id: &str) -> SessionFile {
        todo!()
    }
    /// TS adapter.ts:1432.
    pub async fn session_held(&self, thread_id: &str, exec: Arc<dyn HarnessExec>) -> Result<String, ExecFailure> {
        todo!()
    }
    /// TS adapter.ts:1433.
    pub async fn rename_session(&self, thread_id: &str, title: &str, exec: Arc<dyn HarnessExec>) -> Result<SessionRenameWrite, ExecFailure> {
        todo!()
    }
    /// TS adapter.ts:1434.
    pub async fn title_for(&self, turn: TitleTurn, exec: Arc<dyn HarnessExec>) -> Result<Option<String>, ExecFailure> {
        todo!()
    }
    /// TS adapter.ts:1443.
    pub async fn draft_for(&self, ask: DraftAsk, exec: Arc<dyn HarnessExec>) -> Result<Option<String>, ExecFailure> {
        todo!()
    }
    /// TS adapter.ts:1333.
    pub fn conversations(&self) -> Arc<CodexConversations> {
        todo!()
    }
}
