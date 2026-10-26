// SPDX-License-Identifier: AGPL-3.0-only
//! The Claude Code adapter: its launches, its probes and questions, and the HarnessAdapter the runtime reads. TS:
//! packages/adapter-claude/src/adapter.ts:1-212, 627-645 and 1272-1476 at a3b870a6e.

use std::sync::{Arc, Mutex};

use futures::future::LocalBoxFuture;
use indexmap::IndexMap;

use crate::claude::landmines::{BaseEnv, BrokeredSignIn};
use crate::claude::turn::{ClaudeKept, ClaudeSession};
use crate::port::{
    AdapterAttachOptions, AdapterError, AgentLaunch, AsideAnswer, AsideQuestion, ConversationStore, DraftAsk, Env, ExecFailure,
    ExecStreamFactory, ForkTurn, ForkedSession, HarnessCatalogProbe, HarnessExec, HarnessStartOptions, HarnessWarmOptions, Now,
    ScreenCommand, SessionFile, SessionRenameWrite, TitleTurn,
};

/// adapter.ts:117.
#[derive(Clone)]
pub struct AdapterDeps {
    pub exec: Arc<dyn ExecStreamFactory>,
    /// Must be absolute once trimmed.
    pub config_dir: String,
    pub base_env: Option<BaseEnv>,
    pub project_dir_name: Option<String>,
    pub api_key: Option<String>,
    pub oauth_token: Option<String>,
    pub brokered: Option<BrokeredSignIn>,
    /// INTERRUPT_GRACE_MS (5000) where `None`.
    pub interrupt_grace_ms: Option<u64>,
    /// RUN_EXIT_MS (10000) where `None`.
    pub result_exit_ms: Option<u64>,
    /// STEER_JOIN_MS (10000) where `None`.
    pub steer_join_ms: Option<u64>,
    pub sign_in_refusal: Option<String>,
    /// ASIDE_WALL_MS (180000) where `None`.
    pub aside_wall_ms: Option<u64>,
    pub launch: Option<AgentLaunch>,
    /// STOP_TASK_WAIT_MS where `None`.
    pub stop_task_wait_ms: Option<u64>,
    /// The system clock where `None`.
    pub now: Option<Now>,
}

impl AdapterDeps {
    /// Deps with every optional field `None`.
    pub fn new(exec: Arc<dyn ExecStreamFactory>, config_dir: impl Into<String>) -> Self {
        AdapterDeps {
            exec,
            config_dir: config_dir.into(),
            base_env: None,
            project_dir_name: None,
            api_key: None,
            oauth_token: None,
            brokered: None,
            interrupt_grace_ms: None,
            result_exit_ms: None,
            steer_join_ms: None,
            sign_in_refusal: None,
            aside_wall_ms: None,
            launch: None,
            stop_task_wait_ms: None,
            now: None,
        }
    }
}

/// What every turn of one adapter shares: its deps and what they fix (the environment, the sign-in guard, the memory
/// folder) and the sessions map. Fields are the implementer's to extend.
pub struct AdapterCore {
    pub deps: AdapterDeps,
    pub env: Env,
    pub guard: String,
    pub memory_dir: Option<String>,
    pub sessions: Mutex<IndexMap<String, ClaudeSession>>,
}

/// What attach found. adapter.ts:162.
pub enum AttachedSession {
    Session(ClaudeSession),
    Gone,
}

/// adapter.ts:154.
#[derive(Clone)]
pub struct ClaudeAdapter {
    core: Arc<AdapterCore>,
}

/// adapter.ts:627. `Err("configDir must be an absolute path, got \"<dir>\"")` where it is not.
pub fn create_claude_adapter(deps: AdapterDeps) -> Result<ClaudeAdapter, AdapterError> {
    todo!()
}

impl ClaudeAdapter {
    /// adapter.ts:1312. `Err` where the launch line refuses a value (a slug, a window, a session id).
    pub fn start(&self, options: HarnessStartOptions) -> Result<ClaudeSession, AdapterError> {
        todo!()
    }
    /// adapter.ts:1298.
    pub fn warm(&self, options: HarnessWarmOptions) -> Result<ClaudeKept, AdapterError> {
        todo!()
    }
    /// adapter.ts:1426. `None` where the exec factory has no attach.
    pub fn attach(&self, options: AdapterAttachOptions) -> Option<LocalBoxFuture<'static, Result<AttachedSession, ExecFailure>>> {
        todo!()
    }
    /// A snapshot of the sessions map, in the order they were registered.
    pub fn sessions(&self) -> IndexMap<String, ClaudeSession> {
        todo!()
    }
    /// adapter.ts:1438.
    pub fn terminal_resume(&self) -> String {
        todo!()
    }
    /// adapter.ts:1444.
    pub fn screen_commands(&self) -> Vec<ScreenCommand> {
        todo!()
    }
    /// adapter.ts:1473: what every turn's command is exported with.
    pub fn env(&self) -> Env {
        todo!()
    }
    /// adapter.ts:1445.
    pub async fn probe_catalog(&self, exec: Arc<dyn HarnessExec>) -> Result<Option<HarnessCatalogProbe>, ExecFailure> {
        todo!()
    }
    /// adapter.ts:1446.
    pub async fn probe_version(&self, exec: Arc<dyn HarnessExec>) -> Result<Option<String>, ExecFailure> {
        todo!()
    }
    /// adapter.ts:1447.
    pub async fn session_title(&self, session_id: &str, exec: Arc<dyn HarnessExec>) -> Result<Option<String>, ExecFailure> {
        todo!()
    }
    /// adapter.ts:1448.
    pub fn session_file(&self, session_id: &str) -> SessionFile {
        todo!()
    }
    /// adapter.ts:1449.
    pub async fn rename_session(
        &self,
        session_id: &str,
        title: &str,
        exec: Arc<dyn HarnessExec>,
    ) -> Result<SessionRenameWrite, ExecFailure> {
        todo!()
    }
    /// adapter.ts:1451.
    pub async fn title_for(&self, turn: TitleTurn, exec: Arc<dyn HarnessExec>) -> Result<Option<String>, ExecFailure> {
        todo!()
    }
    /// adapter.ts:1461.
    pub async fn draft_for(&self, ask: DraftAsk, exec: Arc<dyn HarnessExec>) -> Result<Option<String>, ExecFailure> {
        todo!()
    }
    /// adapter.ts:1366. `Err` with the words the TS rejects with.
    pub async fn aside(&self, question: AsideQuestion) -> Result<AsideAnswer, String> {
        todo!()
    }
    /// adapter.ts:1357. A turn with no anchor is refused with "claude forks a session at the reply a turn ended on,
    /// and this turn named none".
    pub async fn fork_session(&self, session: &str, turn: ForkTurn) -> Result<ForkedSession, AdapterError> {
        todo!()
    }
    /// adapter.ts:1474.
    pub fn conversations(&self) -> Arc<dyn ConversationStore> {
        todo!()
    }
}

/// The runtime's view of the Claude adapter (spec/claude_adapter.md, Construction): every capability Claude Code
/// declares, and each optional TS member present.
impl crate::port::HarnessAdapter for ClaudeAdapter {
    fn start(&self, options: HarnessStartOptions) -> Result<Arc<dyn crate::port::HarnessSession>, AdapterError> {
        todo!()
    }
    fn warm(&self, options: HarnessWarmOptions) -> Option<Result<Box<dyn crate::port::KeptAgent>, AdapterError>> {
        todo!()
    }
    fn attach(
        &self,
        options: AdapterAttachOptions,
    ) -> Option<LocalBoxFuture<'static, Result<Option<Arc<dyn crate::port::HarnessSession>>, ExecFailure>>> {
        todo!()
    }
    fn steers(&self) -> bool {
        todo!()
    }
    fn reports_edits(&self) -> bool {
        todo!()
    }
    fn fork_session(&self, session: String, turn: ForkTurn) -> Option<LocalBoxFuture<'static, Result<ForkedSession, String>>> {
        todo!()
    }
    fn attachments(&self) -> Option<crate::port::AttachmentRoad> {
        todo!()
    }
    fn mcp_servers(&self) -> bool {
        todo!()
    }
    fn denies_servers(&self) -> bool {
        todo!()
    }
    fn screen_commands(&self) -> Vec<ScreenCommand> {
        todo!()
    }
    fn moves_access(&self) -> bool {
        todo!()
    }
    fn steers_images(&self) -> bool {
        todo!()
    }
    fn waits_for_prompt(&self) -> bool {
        todo!()
    }
    fn probe_catalog(
        &self,
        exec: Arc<dyn HarnessExec>,
    ) -> Option<LocalBoxFuture<'static, Result<Option<crate::port::HarnessCatalogAnswer>, ExecFailure>>> {
        todo!()
    }
    fn probe_version(&self, exec: Arc<dyn HarnessExec>) -> Option<LocalBoxFuture<'static, Result<Option<String>, ExecFailure>>> {
        todo!()
    }
    fn session_title(
        &self,
        harness_session_id: String,
        exec: Arc<dyn HarnessExec>,
    ) -> Option<LocalBoxFuture<'static, Result<Option<String>, ExecFailure>>> {
        todo!()
    }
    fn session_file(&self, session_id: &str) -> Option<SessionFile> {
        todo!()
    }
    fn rename_session(
        &self,
        harness_session_id: String,
        title: String,
        exec: Arc<dyn HarnessExec>,
    ) -> Option<LocalBoxFuture<'static, Result<SessionRenameWrite, ExecFailure>>> {
        todo!()
    }
    fn title_for(
        &self,
        turn: TitleTurn,
        exec: Arc<dyn HarnessExec>,
    ) -> Option<LocalBoxFuture<'static, Result<Option<String>, ExecFailure>>> {
        todo!()
    }
    fn draft_for(&self, ask: DraftAsk, exec: Arc<dyn HarnessExec>) -> Option<LocalBoxFuture<'static, Result<Option<String>, ExecFailure>>> {
        todo!()
    }
    fn aside(&self, question: AsideQuestion) -> Option<LocalBoxFuture<'static, Result<AsideAnswer, String>>> {
        todo!()
    }
    fn aside_servers(&self) -> bool {
        todo!()
    }
    fn compacts(&self) -> Option<String> {
        todo!()
    }
    fn terminal_resume(&self) -> Option<String> {
        todo!()
    }
    fn env(&self) -> Option<Env> {
        todo!()
    }
    fn conversations(&self) -> Option<Arc<dyn ConversationStore>> {
        todo!()
    }
}
