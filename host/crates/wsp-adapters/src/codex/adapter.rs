// SPDX-License-Identifier: AGPL-3.0-only
//! The Codex adapter: its launches, side questions, side runs, kept servers, probes and store. TS
//! adapter-codex/src/adapter.ts:104-249 (the types) and 513-519, 1229-1454 (`createCodexAdapter` past `follow`).
//! Spec: spec/codex_adapter.md.

use std::sync::{Arc, Mutex};

use futures::future::{FutureExt, LocalBoxFuture};
use indexmap::IndexMap;
use serde_json::Value;

use super::background::{ended_between, Background};
use super::catalog::{account_models_of, catalog_probe_command, parse_version, read_catalog_probe, version_probe_command};
use super::command::{
    access_params, build_command, build_env, AccountModels, BrokerMode, BuildCommandOptions, CodexBroker, CodexEnvOptions,
};
use super::conversations::{codex_conversations, CodexConversations, CodexRoad};
use super::items::{rollout_in, turn_line_for, ASIDE_INSTRUCTIONS, COMPACT};
use super::lost_session::start_seeded;
use super::rpc::{
    initialize_line, rate_limits_read_line, thread_copy_line, thread_fork_line, thread_resume_line, thread_start_line, turn_start_line,
    ServiceTier, ThreadOptions, ACCOUNT_READ_LINE, INITIALIZED_LINE,
};
use super::session_held::{parse_session_held, session_held_command};
use super::session_title::{
    draft_for_command, parse_draft_for, parse_rename, parse_session_title, parse_title_for, rename_command, session_title_command,
    title_for_command,
};
use super::turn::{follow, plain_failure, CodexKept, CodexSession, FollowOptions, OpenedThread, Sessions, SideRun, TurnContext};
use super::turn_pages::{fork_session_on, revert_on, SideRunner};
use crate::claude::turn::{ASIDE_WALL_MS, INTERRUPT_GRACE_MS, RUN_EXIT_MS, STEER_JOIN_MS};
use crate::pending::title_prompt;
use crate::port::{
    keep_run, started, system_now, AdapterAttachOptions, AdapterError, AgentLaunch, AnswerOutcome, AsideAnswer, AsideQuestion,
    AttachRunOptions, Attached, AttachmentRoad, ConversationStore, DraftAsk, Env, ExecFailure, ExecFailureKind, ExecOptions, ExecSecret,
    ExecStreamFactory, ForkTurn, ForkedSession, HarnessAdapter, HarnessCatalogAnswer, HarnessCatalogModelProbe, HarnessExec,
    HarnessSession, HarnessStartOptions, KeptAgent, KeptTurn, Now, PermissionAnswer, RevertAsk, Reverted, SessionFile, SessionRenameWrite,
    SharedFuture, SteerOutcome, TaskStop, TitleTurn, TurnImage, TurnResult, TurnStatus,
};
use wsp_wire::views::session_events::TurnTokens;

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
        CodexAdapterDeps {
            exec,
            home: home.into(),
            login: login.into(),
            api_key: None,
            key_env: None,
            base_env: None,
            interrupt_grace_ms: None,
            result_exit_ms: None,
            steer_join_ms: None,
            reconnect_stall_ms: None,
            aside_wall_ms: None,
            launch: None,
            broker: None,
            now: None,
        }
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
    ctx: TurnContext,
    /// The environment a question outside a turn reads: the turn's own where brokered, else build_env without a key.
    question_env: Env,
}

/// TS adapter.ts:513. `Err` where build_env refuses (a relative home, a key beside a ChatGPT broker, a key that is
/// not one plain word).
pub fn create_codex_adapter(deps: CodexAdapterDeps) -> Result<CodexAdapter, AdapterError> {
    let env = build_env(&CodexEnvOptions {
        base: deps.base_env.clone(),
        home: deps.home.clone(),
        api_key: deps.api_key.clone(),
        broker: deps.broker.clone(),
    })
    .map_err(AdapterError)?;
    let question_env = if deps.broker.is_some() {
        env.clone()
    } else {
        build_env(&CodexEnvOptions { base: deps.base_env.clone(), home: deps.home.clone(), ..CodexEnvOptions::default() })
            .map_err(AdapterError)?
    };
    let ctx = TurnContext {
        login: deps.login.clone(),
        key_env: deps.key_env.clone(),
        home: deps.home.clone(),
        grace_ms: deps.interrupt_grace_ms.unwrap_or(INTERRUPT_GRACE_MS),
        result_exit_ms: deps.result_exit_ms.unwrap_or(RUN_EXIT_MS),
        steer_join_ms: deps.steer_join_ms.unwrap_or(STEER_JOIN_MS),
        reconnect_stall_ms: deps.reconnect_stall_ms.unwrap_or(super::items::RECONNECT_STALL_MS),
        aside_wall_ms: deps.aside_wall_ms.unwrap_or(ASIDE_WALL_MS),
        now: deps.now.clone().unwrap_or_else(system_now),
    };
    Ok(CodexAdapter { deps, env, sessions: Arc::new(Mutex::new(IndexMap::new())), ctx, question_env })
}

/// An event sink that drops what it is given. TS adapter.ts:1305 (`onEvent: () => {}`).
fn quiet() -> crate::port::OnEvent {
    Arc::new(|_: crate::port::AdapterEvent| {})
}

/// A fresh registry key or session id. TS adapter.ts:1299 (`randomUUID()`).
fn new_id() -> String {
    uuid::Uuid::new_v4().to_string()
}

/// A failure a command builder or a store read raised before anything ran: the TS throw, as the port's `ExecFailure`.
fn other(message: String) -> ExecFailure {
    ExecFailure { kind: ExecFailureKind::Other, message }
}

impl CodexAdapter {
    /// The capability flags of adapter.ts:1392-1427, each `true` there.
    pub fn steers(&self) -> bool {
        true
    }
    pub fn reports_edits(&self) -> bool {
        true
    }
    pub fn mcp_servers(&self) -> bool {
        true
    }
    pub fn denies_servers(&self) -> bool {
        true
    }
    pub fn waits_for_prompt(&self) -> bool {
        true
    }
    pub fn forks_by_count(&self) -> bool {
        true
    }
    /// `file`: each image lands on the machine first. TS adapter.ts:1394.
    pub fn attachments(&self) -> Option<AttachmentRoad> {
        Some(AttachmentRoad::File)
    }
    /// TS adapter.ts:1298 (`start` through `startSeeded` and `launchTurn`). `Err` where the TS throws before anything
    /// is launched.
    pub fn start(&self, options: HarnessStartOptions) -> Result<CodexSession, AdapterError> {
        let me = self.clone();
        let launch: Arc<dyn Fn(HarnessStartOptions) -> Result<CodexSession, AdapterError>> = Arc::new(move |o| me.launch_turn(o));
        start_seeded(launch, &self.sessions, options)
    }

    /// TS adapter.ts:1281 (`launchTurn`): the thread line and the server's command, launched and read as one turn.
    fn launch_turn(&self, options: HarnessStartOptions) -> Result<CodexSession, AdapterError> {
        if options.context_window.is_some() {
            return Err(AdapterError("codex takes no context window".to_string()));
        }
        let images =
            options.images.iter().map(super::turn::image_path_of).collect::<Result<Vec<String>, String>>().map_err(AdapterError)?;
        let access = access_params(options.permission_mode.as_deref()).map_err(AdapterError)?;
        let local_id = match (options.copy, &options.resume) {
            (true, _) => new_id(),
            (false, Some(resume)) => resume.clone(),
            (false, None) => new_id(),
        };
        let config = options.server_values.as_ref().and_then(|v| v.config.clone()).unwrap_or_default();
        let valued = !config.is_empty();
        let thread = ThreadOptions {
            cwd: options.cwd.clone(),
            model: options.model.clone(),
            service_tier: (options.fast == Some(true)).then_some(ServiceTier::Fast),
            access,
            config: valued.then(|| config.clone()),
        };
        if options.fork.is_some() && options.resume.is_some() {
            return Err(AdapterError("a codex start forks a thread or resumes one, never both".to_string()));
        }
        let thread_line = match (&options.fork, &options.resume) {
            (Some((session, turn)), _) => thread_copy_line(&thread, session, Some(turn)),
            (None, None) => thread_start_line(&thread),
            (None, Some(resume)) if options.copy => thread_copy_line(&thread, resume, None),
            (None, Some(resume)) => thread_resume_line(&thread, resume),
        };
        let command = build_command(
            &BuildCommandOptions {
                cwd: options.cwd.clone(),
                mcp_servers: options.mcp_servers.clone(),
                launch: self.deps.launch.clone(),
                broker: self.deps.broker.clone(),
                off_servers: options.off_servers.clone(),
                side_run: false,
            },
            (self.ctx.now)(),
        )
        .map_err(AdapterError)?;
        let input = vec![
            initialize_line(),
            INITIALIZED_LINE.to_string(),
            ACCOUNT_READ_LINE.to_string(),
            rate_limits_read_line(options.limit_details),
            thread_line,
        ];
        let run = self.deps.exec.launch(
            &command,
            ExecOptions {
                env: self.env.clone(),
                input: Some(input),
                input_after: None,
                secret: valued.then(|| ExecSecret { files: None, input: true }),
            },
        );
        let background = options.keep.then(|| Arc::new(Mutex::new(Background::default())));
        let keeper = background.as_ref().map(|bg| {
            let bg = bg.clone();
            let reader = run.clone();
            let between: Arc<dyn Fn(String)> = Arc::new(move |line: String| {
                ended_between(reader.as_ref(), &mut bg.lock().unwrap(), &line);
            });
            keep_run(run.clone(), Some(between))
        });
        let stream = match &keeper {
            Some(keeper) => keeper.turn().0,
            None => run.clone(),
        };
        let prompt = options.prompt.clone();
        let effort = options.effort.clone();
        let turn_line: Box<dyn Fn(&str) -> String> =
            Box::new(move |thread_id: &str| turn_line_for(thread_id, &prompt, &images, effort.as_deref()));
        Ok(follow(
            &self.ctx,
            &self.sessions,
            FollowOptions {
                stream,
                local_id,
                started_at: (self.ctx.now)(),
                command: Some(command),
                model: None,
                cwd: options.cwd.clone(),
                turn_line: Some(turn_line),
                prompt_after: options.prompt_after,
                owed_turn: None,
                aside: false,
                side_run: None,
                keeper,
                background,
                from: None,
                opened: None,
                thread_options: Some(thread),
                on_event: options.on_event.clone().unwrap_or_else(quiet),
            },
        ))
    }

    /// Re-opens a turn a host that went before still runs on the machine. TS adapter.ts:1397.
    pub fn attach(&self, options: AdapterAttachOptions) -> Option<LocalBoxFuture<'static, Result<AttachedSession, ExecFailure>>> {
        let pending =
            self.deps.exec.attach(&options.run, AttachRunOptions { input: true, started_at: options.started_at, from: options.from })?;
        let me = self.clone();
        Some(
            async move {
                match pending.await? {
                    Attached::Gone => Ok(AttachedSession::Gone),
                    Attached::Stream(stream) => {
                        let from = options.from.filter(|from| *from > 0);
                        let opened = from.map(|_| OpenedThread { thread_id: options.session_id.clone(), legacy: false });
                        let prompt = options.prompt.clone();
                        let effort = options.effort.clone();
                        let owed_turn: Option<Box<dyn Fn(&str) -> String>> = prompt.map(|prompt| {
                            Box::new(move |thread_id: &str| turn_line_for(thread_id, &prompt, &[], effort.as_deref()))
                                as Box<dyn Fn(&str) -> String>
                        });
                        Ok(AttachedSession::Session(follow(
                            &me.ctx,
                            &me.sessions,
                            FollowOptions {
                                stream,
                                local_id: options.session_id.clone(),
                                started_at: options.started_at as i64,
                                command: None,
                                model: options.model.clone(),
                                cwd: options.cwd.clone(),
                                turn_line: None,
                                prompt_after: None,
                                owed_turn,
                                aside: false,
                                side_run: None,
                                keeper: None,
                                background: None,
                                from,
                                opened,
                                thread_options: None,
                                on_event: options.on_event.clone(),
                            },
                        )))
                    }
                }
            }
            .boxed_local(),
        )
    }

    /// A snapshot of the sessions map, in registration order.
    pub fn sessions(&self) -> IndexMap<String, CodexSession> {
        crate::codex::turn::live(&self.sessions)
    }
    /// The turn's environment. TS adapter.ts:1452.
    pub fn env(&self) -> Env {
        self.env.clone()
    }
    /// `/compact`. TS adapter.ts:1418.
    pub fn compacts(&self) -> &'static str {
        COMPACT
    }

    /// The launch a question or a side run takes: the person's program alone, never its arguments. TS adapter.ts:1303.
    fn program_launch(&self) -> Option<AgentLaunch> {
        self.deps.launch.as_ref().and_then(|l| l.program.clone()).map(|program| AgentLaunch { program: Some(program), args: None })
    }

    /// The account's models, where the host holds the ChatGPT sign-in for this machine. TS adapter.ts:1389.
    fn account_models_source(&self) -> Option<AccountModels> {
        match self.deps.broker.as_ref().map(|b| &b.mode) {
            Some(BrokerMode::Chatgpt { models, .. }) => models.clone(),
            _ => None,
        }
    }

    /// A question put to a copy of the thread on an ephemeral fork. TS adapter.ts:1303.
    pub async fn aside(&self, question: AsideQuestion) -> Result<AsideAnswer, String> {
        let command = build_command(
            &BuildCommandOptions {
                cwd: question.cwd.clone(),
                launch: self.program_launch(),
                broker: self.deps.broker.clone(),
                ..BuildCommandOptions::default()
            },
            (self.ctx.now)(),
        )?;
        let fork = thread_fork_line(&question.session, question.cwd.as_deref(), question.model.as_deref(), ASIDE_INSTRUCTIONS);
        let stream = self.deps.exec.launch(
            &command,
            ExecOptions {
                env: self.env.clone(),
                input: Some(vec![initialize_line(), INITIALIZED_LINE.to_string(), fork]),
                ..ExecOptions::default()
            },
        );
        let text = question.question.clone();
        let turn_line: Box<dyn Fn(&str) -> String> = Box::new(move |thread_id: &str| turn_start_line(thread_id, &text, &[], None));
        let result = follow(
            &self.ctx,
            &self.sessions,
            FollowOptions {
                stream,
                local_id: new_id(),
                started_at: (self.ctx.now)(),
                command: Some(command),
                model: None,
                cwd: None,
                turn_line: Some(turn_line),
                prompt_after: None,
                owed_turn: None,
                aside: true,
                side_run: None,
                keeper: None,
                background: None,
                from: None,
                opened: None,
                thread_options: None,
                on_event: quiet(),
            },
        )
        .finished()
        .await;
        if result.status != TurnStatus::Completed {
            return Err(result.error.unwrap_or_else(|| "codex did not answer the question".to_string()));
        }
        Ok(AsideAnswer { text: result.text.unwrap_or_default(), usage: result.tokens.map(|tokens| tokens_map(&tokens)) })
    }

    /// A server run of its own that loads what its input asks and runs no turn. TS adapter.ts:1320.
    fn side_runner(&self) -> SideRunner {
        let me = self.clone();
        Box::new(move |cwd: Option<String>, input: Vec<String>, side_run: SideRun| {
            let command = match build_command(
                &BuildCommandOptions {
                    cwd: cwd.clone(),
                    launch: me.program_launch(),
                    broker: me.deps.broker.clone(),
                    side_run: true,
                    ..BuildCommandOptions::default()
                },
                (me.ctx.now)(),
            ) {
                Ok(command) => command,
                Err(error) => return async move { plain_failure(error) }.boxed_local(),
            };
            let mut lines = vec![initialize_line(), INITIALIZED_LINE.to_string()];
            lines.extend(input);
            let stream = me.deps.exec.launch(&command, ExecOptions { env: me.env.clone(), input: Some(lines), ..ExecOptions::default() });
            follow(
                &me.ctx,
                &me.sessions,
                FollowOptions {
                    stream,
                    local_id: new_id(),
                    started_at: (me.ctx.now)(),
                    command: Some(command),
                    model: None,
                    cwd: None,
                    turn_line: None,
                    prompt_after: None,
                    owed_turn: None,
                    aside: false,
                    side_run: Some(side_run),
                    keeper: None,
                    background: None,
                    from: None,
                    opened: None,
                    thread_options: None,
                    on_event: quiet(),
                },
            )
            .finished()
            .boxed_local()
        })
    }

    /// TS adapter.ts:1324.
    pub async fn revert(&self, ask: RevertAsk) -> Result<Reverted, String> {
        revert_on(&self.side_runner(), ask).await
    }

    /// TS adapter.ts:1325. The session and turn the fork's start runs on.
    pub async fn fork_session(&self, session: &str, turn: ForkTurn) -> Result<(String, String), String> {
        fork_session_on(&self.side_runner(), session.to_string(), turn).await
    }

    /// TS adapter.ts:1390.
    pub async fn probe_catalog(&self, exec: Arc<dyn HarnessExec>) -> Result<Option<HarnessCatalogAnswer>, ExecFailure> {
        let command = catalog_probe_command(self.deps.launch.as_ref(), self.deps.broker.as_ref()).map_err(other)?;
        let stdout = exec.exec(&command, Some(&self.question_env)).await?;
        let models = self.account_models_source();
        Ok(read_catalog_probe(&stdout, &self.deps.login, models.as_ref()).await)
    }

    /// TS adapter.ts:1429.
    pub async fn probe_version(&self, exec: Arc<dyn HarnessExec>) -> Result<Option<String>, ExecFailure> {
        let command = version_probe_command(self.deps.launch.as_ref(), self.deps.broker.as_ref()).map_err(other)?;
        let stdout = exec.exec(&command, Some(&self.question_env)).await?;
        Ok(parse_version(&stdout))
    }

    /// `None` at the outer level where the host holds no ChatGPT sign-in for the machine. TS adapter.ts:1428.
    pub fn account_models(&self, version: Option<String>) -> Option<LocalBoxFuture<'static, Option<Vec<HarnessCatalogModelProbe>>>> {
        let models = self.account_models_source()?;
        Some(async move { account_models_of(&models, version.as_deref()).await }.boxed_local())
    }

    /// TS adapter.ts:1430.
    pub async fn session_title(&self, thread_id: &str, exec: Arc<dyn HarnessExec>) -> Result<Option<String>, ExecFailure> {
        let command = session_title_command(&self.deps.home, thread_id).map_err(other)?;
        Ok(parse_session_title(&exec.exec(&command, None).await?))
    }

    /// TS adapter.ts:1431.
    pub fn session_file(&self, thread_id: &str) -> SessionFile {
        rollout_in(&self.deps.home, thread_id)
    }

    /// TS adapter.ts:1432.
    pub async fn session_held(&self, thread_id: &str, exec: Arc<dyn HarnessExec>) -> Result<String, ExecFailure> {
        let command = session_held_command(&self.deps.home, thread_id).map_err(other)?;
        Ok(parse_session_held(&exec.exec(&command, None).await?))
    }

    /// TS adapter.ts:1433.
    pub async fn rename_session(
        &self,
        thread_id: &str,
        title: &str,
        exec: Arc<dyn HarnessExec>,
    ) -> Result<SessionRenameWrite, ExecFailure> {
        let command = rename_command(thread_id, title, self.deps.launch.as_ref(), self.deps.broker.as_ref()).map_err(other)?;
        Ok(parse_rename(&exec.exec(&command, Some(&self.question_env)).await?))
    }

    /// TS adapter.ts:1434.
    pub async fn title_for(&self, turn: TitleTurn, exec: Arc<dyn HarnessExec>) -> Result<Option<String>, ExecFailure> {
        let command = title_for_command(
            &title_prompt(&turn.opening, turn.reply.as_deref()),
            turn.model.as_deref(),
            self.deps.launch.as_ref(),
            self.deps.broker.as_ref(),
            (self.ctx.now)(),
        )
        .map_err(other)?;
        Ok(parse_title_for(&exec.exec(&command, Some(&self.question_env)).await?))
    }

    /// TS adapter.ts:1443.
    pub async fn draft_for(&self, ask: DraftAsk, exec: Arc<dyn HarnessExec>) -> Result<Option<String>, ExecFailure> {
        let command = draft_for_command(
            &ask.prompt_file,
            ask.model.as_deref(),
            self.deps.launch.as_ref(),
            self.deps.broker.as_ref(),
            (self.ctx.now)(),
        )
        .map_err(other)?;
        Ok(parse_draft_for(&exec.exec(&command, Some(&self.question_env)).await?))
    }

    /// TS adapter.ts:1333.
    pub fn conversations(&self) -> Arc<CodexConversations> {
        let road = CodexRoad { launch: self.deps.launch.clone(), broker: self.deps.broker.clone() };
        codex_conversations(&self.deps.home, road, self.question_env.clone())
    }
}

/// A usage object as the harness wrote it, in the order TurnTokens serializes its fields. TS adapter.ts:1305.
fn tokens_map(tokens: &TurnTokens) -> IndexMap<String, Value> {
    match serde_json::to_value(tokens) {
        Ok(Value::Object(map)) => map.into_iter().collect(),
        _ => IndexMap::new(),
    }
}

/// The runtime's view of the Codex adapter: the capabilities and optional members of adapter.ts:1392-1454.
impl HarnessAdapter for CodexAdapter {
    fn start(&self, options: HarnessStartOptions) -> Result<Arc<dyn HarnessSession>, AdapterError> {
        CodexAdapter::start(self, options).map(|session| Arc::new(session) as Arc<dyn HarnessSession>)
    }
    fn renames(&self) -> bool {
        true
    }
    fn asides(&self) -> bool {
        true
    }
    fn session_held(&self, session_id: String, exec: Arc<dyn HarnessExec>) -> Option<LocalBoxFuture<'static, Result<String, String>>> {
        let me = self.clone();
        Some(started(async move { me.session_held(&session_id, exec).await.map_err(|e| e.to_string()) }.boxed_local()))
    }
    fn account_models(
        &self,
        version: Option<String>,
    ) -> Option<LocalBoxFuture<'static, Result<Option<Vec<HarnessCatalogModelProbe>>, String>>> {
        CodexAdapter::account_models(self, version).map(|models| models.map(Ok).boxed_local())
    }
    fn attaches(&self) -> bool {
        self.deps.exec.attaches()
    }
    fn holds_session(&self) -> bool {
        true
    }
    fn attach(
        &self,
        options: AdapterAttachOptions,
    ) -> Option<LocalBoxFuture<'static, Result<Option<Arc<dyn HarnessSession>>, ExecFailure>>> {
        CodexAdapter::attach(self, options).map(|pending| {
            async move {
                Ok(match pending.await? {
                    AttachedSession::Session(session) => Some(Arc::new(session) as Arc<dyn HarnessSession>),
                    AttachedSession::Gone => None,
                })
            }
            .boxed_local()
        })
    }
    fn steers(&self) -> bool {
        true
    }
    fn reports_edits(&self) -> bool {
        true
    }
    fn reverts(&self) -> bool {
        true
    }
    fn forks(&self) -> bool {
        true
    }
    fn revert(&self, ask: RevertAsk) -> Option<LocalBoxFuture<'static, Result<Reverted, String>>> {
        let me = self.clone();
        Some(started(async move { me.revert(ask).await }.boxed_local()))
    }
    fn fork_session(&self, session: String, turn: ForkTurn) -> Option<LocalBoxFuture<'static, Result<ForkedSession, String>>> {
        let me = self.clone();
        Some(started(
            async move { me.fork_session(&session, turn).await.map(|(session, turn)| ForkedSession::Fork { session, turn }) }.boxed_local(),
        ))
    }
    fn forks_by_count(&self) -> bool {
        true
    }
    fn attachments(&self) -> Option<AttachmentRoad> {
        CodexAdapter::attachments(self)
    }
    fn mcp_servers(&self) -> bool {
        true
    }
    fn denies_servers(&self) -> bool {
        true
    }
    fn waits_for_prompt(&self) -> bool {
        true
    }
    fn probe_catalog(
        &self,
        exec: Arc<dyn HarnessExec>,
    ) -> Option<LocalBoxFuture<'static, Result<Option<HarnessCatalogAnswer>, ExecFailure>>> {
        let me = self.clone();
        Some(started(async move { me.probe_catalog(exec).await }.boxed_local()))
    }
    fn probe_version(&self, exec: Arc<dyn HarnessExec>) -> Option<LocalBoxFuture<'static, Result<Option<String>, ExecFailure>>> {
        let me = self.clone();
        Some(started(async move { me.probe_version(exec).await }.boxed_local()))
    }
    fn session_title(
        &self,
        harness_session_id: String,
        exec: Arc<dyn HarnessExec>,
    ) -> Option<LocalBoxFuture<'static, Result<Option<String>, ExecFailure>>> {
        let me = self.clone();
        Some(started(async move { me.session_title(&harness_session_id, exec).await }.boxed_local()))
    }
    fn session_file(&self, session_id: &str) -> Option<SessionFile> {
        Some(CodexAdapter::session_file(self, session_id))
    }
    fn rename_session(
        &self,
        harness_session_id: String,
        title: String,
        exec: Arc<dyn HarnessExec>,
    ) -> Option<LocalBoxFuture<'static, Result<SessionRenameWrite, ExecFailure>>> {
        let me = self.clone();
        Some(started(async move { me.rename_session(&harness_session_id, &title, exec).await }.boxed_local()))
    }
    fn title_for(
        &self,
        turn: TitleTurn,
        exec: Arc<dyn HarnessExec>,
    ) -> Option<LocalBoxFuture<'static, Result<Option<String>, ExecFailure>>> {
        let me = self.clone();
        Some(started(async move { me.title_for(turn, exec).await }.boxed_local()))
    }
    fn draft_for(&self, ask: DraftAsk, exec: Arc<dyn HarnessExec>) -> Option<LocalBoxFuture<'static, Result<Option<String>, ExecFailure>>> {
        let me = self.clone();
        Some(started(async move { me.draft_for(ask, exec).await }.boxed_local()))
    }
    fn aside(&self, question: AsideQuestion) -> Option<LocalBoxFuture<'static, Result<AsideAnswer, String>>> {
        let me = self.clone();
        Some(started(async move { me.aside(question).await }.boxed_local()))
    }
    fn compacts(&self) -> Option<String> {
        Some(CodexAdapter::compacts(self).to_string())
    }
    fn env(&self) -> Option<Env> {
        Some(CodexAdapter::env(self))
    }
    fn conversations(&self) -> Option<Arc<dyn ConversationStore>> {
        Some(CodexAdapter::conversations(self))
    }
}

impl HarnessSession for CodexSession {
    fn local_id(&self) -> String {
        CodexSession::local_id(self)
    }
    fn finished(&self) -> SharedFuture<TurnResult> {
        CodexSession::finished(self)
    }
    fn run(&self) -> Option<String> {
        CodexSession::run(self)
    }
    fn pid(&self) -> Option<u32> {
        CodexSession::pid(self)
    }
    fn from(&self) -> Option<u64> {
        CodexSession::from(self)
    }
    fn interrupt(&self) -> LocalBoxFuture<'static, ()> {
        CodexSession::interrupt(self)
    }
    fn kept(&self) -> Option<Option<Box<dyn KeptAgent>>> {
        Some(CodexSession::kept(self).map(|kept| Box::new(kept) as Box<dyn KeptAgent>))
    }
    /// The server takes no image or id on a steer, so both are dropped as the TS steer drops them.
    fn steer(
        &self,
        prompt: String,
        _id: Option<String>,
        _images: Vec<TurnImage>,
    ) -> Option<LocalBoxFuture<'static, Result<SteerOutcome, ExecFailure>>> {
        Some(CodexSession::steer(self, prompt).map(Ok).boxed_local())
    }
    fn answers(&self) -> bool {
        true
    }
    fn answer(&self, ask_id: String, answer: PermissionAnswer) -> Option<LocalBoxFuture<'static, Result<AnswerOutcome, ExecFailure>>> {
        Some(CodexSession::answer(self, ask_id, answer))
    }
    fn stop_task(&self, task: String) -> Option<LocalBoxFuture<'static, Result<TaskStop, ExecFailure>>> {
        Some(CodexSession::stop_task(self, task).map(Ok).boxed_local())
    }
}

impl KeptAgent for CodexKept {
    fn next(&self, turn: KeptTurn) -> Result<Arc<dyn HarnessSession>, AdapterError> {
        (self.next)(turn).map(|session| Arc::new(session) as Arc<dyn HarnessSession>).map_err(AdapterError)
    }
    fn close(&self, now: bool) -> LocalBoxFuture<'static, ()> {
        (self.close)(now)
    }
    fn exited(&self) -> SharedFuture<()> {
        self.exited.clone().map(|_| ()).boxed_local().shared()
    }
    fn session_file(&self) -> Option<SessionFile> {
        Some(self.session_file.clone())
    }
    fn working(&self) -> Option<bool> {
        Some((self.working)())
    }
}
