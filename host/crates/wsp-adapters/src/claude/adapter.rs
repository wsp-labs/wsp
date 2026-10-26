// SPDX-License-Identifier: AGPL-3.0-only
//! The Claude Code adapter: its launches, its probes and questions, and the HarnessAdapter the runtime reads. TS:
//! packages/adapter-claude/src/adapter.ts:1-212, 627-645 and 1272-1476 at a3b870a6e.

use std::cell::Cell;
use std::rc::Rc;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use futures::future::{FutureExt, LocalBoxFuture};
use futures::stream::StreamExt;
use indexmap::IndexMap;

use crate::claude::aside::{
    aside_answer, aside_command, aside_hooks_line, aside_prompt, aside_text_of, hook_deny_line, prompt_deny_line, AsideCommandOptions,
    ASIDE_HOOKS_ID,
};
use crate::claude::catalog::{catalog_probe_command, claude_screen_commands, parse_catalog_probe, parse_version, version_probe_command};
use crate::claude::chain::{copy_cleanup_command, copy_session, CopyOptions, CopyThrough};
use crate::claude::conversations::{claude_conversations, ConversationDeps};
use crate::claude::fields::{parse_line, string};
use crate::claude::landmines::{
    build_env, launch_command, new_session_id, saved_spend_command, server_values_file, sign_in_guard, terminal_resume_command,
    user_message_line, BaseEnv, BrokeredSignIn, ClaudeEnvOptions, LaunchOptions,
};
use crate::claude::normalize::{drained_notice, session_file_in, NO_CONVERSATION};
use crate::claude::permissions::{control_error_line, control_line, ControlLine};
use crate::claude::session_title::{
    draft_for_command, parse_draft_for, parse_rename, parse_session_title, parse_title_for, rename_command, session_title_command,
    title_for_command,
};
use crate::claude::turn::WeakSession;
use crate::claude::turn::{fire, follow, ClaudeKept, ClaudeSession, FollowOptions, Kept, ASIDE_WALL_MS, INTERRUPT_GRACE_MS, RUN_EXIT_MS};
use crate::pending::{aside_wall_line, claude_memory_dir, harness_exit_line, title_prompt, HarnessEnded};
use crate::port::{
    end_after_result, end_run, keep_run, start_after_loss, started, AccessOutcome, AdapterAttachOptions, AdapterError, AfterLoss,
    AgentLaunch, AnswerOutcome, AsideAnswer, AsideQuestion, AttachRunOptions, Attached, ConversationStore, DraftAsk, Env, ExecFailure,
    ExecFailureKind, ExecOptions, ExecStreamFactory, ForkTurn, ForkedSession, HarnessCatalogAnswer, HarnessCatalogProbe, HarnessExec,
    HarnessStartOptions, HarnessWarmOptions, KeptTurn, LostSessionStart, Now, OnEvent, PermissionAnswer, ScreenCommand, SessionFile,
    SessionRenameWrite, Settle, SharedFuture, SteerOutcome, TaskStop, TitleTurn, TurnImage, TurnResult,
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
    /// Held weakly: a session holds this core, so a strong map would keep every finished turn for as long as the adapter.
    pub sessions: Mutex<IndexMap<String, WeakSession>>,
}

impl AdapterCore {
    /// Registers a session under its id, in place of an earlier one, and forgets the ones nothing holds any more.
    pub(crate) fn register(&self, id: String, session: &ClaudeSession) {
        let mut held = self.sessions.lock().unwrap();
        held.retain(|_, weak| weak.alive());
        held.insert(id, session.downgrade());
    }
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

type SharedSettle = futures::future::Shared<Settle>;

/// adapter.ts:627. `Err("configDir must be an absolute path, got \"<dir>\"")` where it is not.
pub fn create_claude_adapter(deps: AdapterDeps) -> Result<ClaudeAdapter, AdapterError> {
    if !wsp_words::js::js_trim(&deps.config_dir).starts_with('/') {
        return Err(AdapterError(format!("configDir must be an absolute path, got \"{}\"", deps.config_dir)));
    }
    let sign_in = sign_in_of(&deps);
    let env = build_env(&ClaudeEnvOptions { project_dir_name: deps.project_dir_name.clone(), ..sign_in.clone() });
    let guard = sign_in_guard(&sign_in, deps.launch.as_ref());
    let memory_dir = deps.project_dir_name.as_deref().map(|key| claude_memory_dir(&deps.config_dir, key));
    Ok(ClaudeAdapter { core: Arc::new(AdapterCore { deps, env, guard, memory_dir, sessions: Mutex::new(IndexMap::new()) }) })
}

/// adapter.ts:631: the sign-in every turn and question is signed in with, before the project key is set.
fn sign_in_of(deps: &AdapterDeps) -> ClaudeEnvOptions {
    ClaudeEnvOptions {
        base: deps.base_env.clone(),
        api_key: deps.api_key.clone(),
        oauth_token: deps.oauth_token.clone(),
        project_dir_name: None,
        brokered: deps.brokered.clone(),
    }
}

fn grace_of(core: &AdapterCore) -> u64 {
    core.deps.interrupt_grace_ms.unwrap_or(INTERRUPT_GRACE_MS)
}

/// adapter.ts:1284 (`launch`): one launch of the CLI, and the turn that reads it. `resume` is the session it resumes
/// (or copies, where `options.copy`), `after` holds the seed line until it settles.
fn launch(
    core: &Arc<AdapterCore>,
    o: &HarnessStartOptions,
    prompt: &str,
    resume: Option<String>,
    on_event: OnEvent,
    after: Option<SharedSettle>,
) -> Result<ClaudeSession, AdapterError> {
    let (copy_of, resume) = if o.copy { (resume, None) } else { (None, resume) };
    let local_id = resume.clone().unwrap_or_else(new_session_id);
    let valued = server_values_file(o.server_values.as_ref());
    let command = launch_command(
        &LaunchOptions {
            resume: resume.clone(),
            copy_of,
            cwd: o.cwd.clone(),
            model: o.model.clone(),
            effort: o.effort.clone(),
            permission_mode: o.permission_mode.clone(),
            context_window: o.context_window.clone(),
            mcp_servers: o.mcp_servers.clone(),
            off_servers: o.off_servers.clone(),
            fast: o.fast,
            memory_dir: core.memory_dir.clone(),
            launch: core.deps.launch.clone(),
            title: o.title.clone(),
            version: o.version.clone(),
        },
        &local_id,
        valued.is_some(),
    )?;
    let launch_line = match &resume {
        Some(id) => format!("{}{}", saved_spend_command(&core.deps.config_dir, id), command),
        None => command,
    };
    let line = user_message_line(prompt, &local_id, &o.images, None);
    let run = core.deps.exec.launch(
        &format!("{}{}", core.guard, launch_line),
        ExecOptions { env: core.env.clone(), input: Some(vec![line]), input_after: after.map(|a| a.boxed_local()), secret: valued },
    );
    let (stream, keeper) = if o.keep {
        let keeper = keep_run(run, None);
        (keeper.turn().0, Some(keeper))
    } else {
        (run, None)
    };
    Ok(follow(
        core,
        FollowOptions {
            stream,
            local_id,
            announced: false,
            fresh: resume.is_none(),
            command: Some(launch_line),
            keeper,
            from: None,
            saved: None,
            steered: None,
            on_event,
        },
    ))
}

async fn remove_copy(core: &AdapterCore, fork: &str) -> Result<(), ExecFailure> {
    let command =
        copy_cleanup_command(fork, &core.deps.config_dir).map_err(|e| ExecFailure { kind: ExecFailureKind::Other, message: e.0 })?;
    let cleanup = core.deps.exec.launch(&command, ExecOptions { env: core.env.clone(), ..Default::default() });
    let mut lines = cleanup.lines();
    while let Some(line) = lines.next().await {
        line?;
    }
    cleanup.exited().await;
    Ok(())
}

impl ClaudeAdapter {
    fn copy_options(&self, session: &str, through: CopyThrough) -> CopyOptions {
        CopyOptions {
            exec: self.core.deps.exec.clone(),
            env: self.core.env.clone(),
            config_dir: self.core.deps.config_dir.clone(),
            session: session.to_string(),
            through,
            grace_ms: grace_of(&self.core),
            wall_ms: None,
        }
    }

    /// The environment the probes and questions run in: the sign-in alone, without the project key a turn gets.
    fn sign_in_env(&self) -> Env {
        build_env(&sign_in_of(&self.core.deps))
    }

    fn probe_env(&self) -> Env {
        build_env(&ClaudeEnvOptions { base: self.core.deps.base_env.clone(), ..Default::default() })
    }

    /// adapter.ts:1312. `Err` where the launch line refuses a value (a slug, a window, a session id).
    pub fn start(&self, mut options: HarnessStartOptions) -> Result<ClaudeSession, AdapterError> {
        let core = self.core.clone();
        let on_event: OnEvent = options.on_event.clone().unwrap_or_else(|| Arc::new(|_| {}));
        let after: Option<SharedSettle> = options.prompt_after.take().map(|a| a.shared());
        let (Some(resume), Some(seed)) = (options.resume.clone(), options.seed.clone()) else {
            let prompt = options.prompt.clone();
            let resume = options.resume.clone();
            return launch(&core, &options, &prompt, resume, on_event, after);
        };
        let prompt = options.prompt.clone();
        let options = Rc::new(options);
        let launcher = {
            let (core, options, after) = (core.clone(), options.clone(), after);
            move |prompt: String, resume: Option<String>, on_event: OnEvent| -> Result<Arc<ClaudeSession>, AdapterError> {
                launch(&core, &options, &prompt, resume, on_event, after.clone()).map(Arc::new)
            }
        };
        let AfterLoss { first, now, finished } = start_after_loss(
            LostSessionStart { prompt, resume: resume.clone(), seed, on_event },
            format!("{NO_CONVERSATION} {resume}"),
            launcher,
        )?;
        let session = ClaudeSession::delegate(first.local_id(), now, finished);
        core.register(first.local_id(), &session);
        Ok(session)
    }

    /// adapter.ts:1298. Refused as the start is, before anything launches.
    pub fn warm(&self, o: HarnessWarmOptions) -> Result<ClaudeKept, AdapterError> {
        let core = self.core.clone();
        let local_id = new_session_id();
        let valued = server_values_file(o.server_values.as_ref());
        let command = launch_command(
            &LaunchOptions {
                cwd: o.cwd,
                model: o.model,
                effort: o.effort,
                permission_mode: o.permission_mode,
                context_window: o.context_window,
                mcp_servers: o.mcp_servers,
                fast: o.fast,
                memory_dir: core.memory_dir.clone(),
                launch: core.deps.launch.clone(),
                version: o.version,
                ..Default::default()
            },
            &local_id,
            valued.is_some(),
        )?;
        let run = core.deps.exec.launch(
            &format!("{}{}", core.guard, command),
            ExecOptions { env: core.env.clone(), input: Some(Vec::new()), input_after: None, secret: valued },
        );
        let keeper = keep_run(run, None);
        keeper.turn().0.close_input();
        Ok(ClaudeKept::new(Kept {
            core: core.clone(),
            keeper,
            local_id: local_id.clone(),
            session_id: local_id,
            saved: None,
            first: Some(command),
            session_file: None,
        }))
    }

    /// adapter.ts:1426. `None` where the exec factory has no attach.
    pub fn attach(&self, o: AdapterAttachOptions) -> Option<LocalBoxFuture<'static, Result<AttachedSession, ExecFailure>>> {
        let core = self.core.clone();
        let pending = core.deps.exec.attach(&o.run, AttachRunOptions { input: true, started_at: o.started_at, from: o.from })?;
        Some(
            async move {
                match pending.await? {
                    Attached::Gone => Ok(AttachedSession::Gone),
                    Attached::Stream(stream) => Ok(AttachedSession::Session(follow(
                        &core,
                        FollowOptions {
                            stream,
                            local_id: o.session_id.clone(),
                            announced: true,
                            fresh: false,
                            command: None,
                            keeper: None,
                            from: o.from,
                            saved: None,
                            steered: o.steered.clone(),
                            on_event: o.on_event.clone(),
                        },
                    ))),
                }
            }
            .boxed_local(),
        )
    }

    /// A snapshot of the sessions map, in the order they were registered: the sessions something still holds.
    pub fn sessions(&self) -> IndexMap<String, ClaudeSession> {
        self.core.sessions.lock().unwrap().iter().filter_map(|(id, weak)| Some((id.clone(), weak.upgrade()?))).collect()
    }

    /// adapter.ts:1438.
    pub fn terminal_resume(&self) -> String {
        terminal_resume_command(self.core.deps.base_env.as_ref(), self.core.deps.launch.as_ref())
    }

    /// adapter.ts:1444.
    pub fn screen_commands(&self) -> Vec<ScreenCommand> {
        claude_screen_commands()
    }

    /// adapter.ts:1473: what every turn's command is exported with.
    pub fn env(&self) -> Env {
        self.core.env.clone()
    }

    /// adapter.ts:1445.
    pub async fn probe_catalog(&self, exec: Arc<dyn HarnessExec>) -> Result<Option<HarnessCatalogProbe>, ExecFailure> {
        let stdout = exec.exec(&catalog_probe_command(self.core.deps.launch.as_ref()), Some(&self.probe_env())).await?;
        Ok(parse_catalog_probe(&stdout))
    }

    /// adapter.ts:1446.
    pub async fn probe_version(&self, exec: Arc<dyn HarnessExec>) -> Result<Option<String>, ExecFailure> {
        let stdout = exec.exec(&version_probe_command(self.core.deps.launch.as_ref()), Some(&self.probe_env())).await?;
        Ok(parse_version(&stdout))
    }

    /// adapter.ts:1447.
    pub async fn session_title(&self, session_id: &str, exec: Arc<dyn HarnessExec>) -> Result<Option<String>, ExecFailure> {
        let stdout = exec.exec(&session_title_command(&self.core.deps.config_dir, session_id), None).await?;
        Ok(parse_session_title(&stdout))
    }

    /// adapter.ts:1448.
    pub fn session_file(&self, session_id: &str) -> SessionFile {
        session_file_in(&self.core.deps.config_dir, session_id)
    }

    /// adapter.ts:1449.
    pub async fn rename_session(
        &self,
        session_id: &str,
        title: &str,
        exec: Arc<dyn HarnessExec>,
    ) -> Result<SessionRenameWrite, ExecFailure> {
        let stdout = exec.exec(&rename_command(&self.core.deps.config_dir, session_id, title), None).await?;
        Ok(parse_rename(&stdout))
    }

    /// adapter.ts:1451.
    pub async fn title_for(&self, turn: TitleTurn, exec: Arc<dyn HarnessExec>) -> Result<Option<String>, ExecFailure> {
        let command = title_for_command(
            &title_prompt(&turn.opening, turn.reply.as_deref()),
            turn.model.as_deref(),
            self.core.deps.launch.as_ref(),
            Some(&self.core.guard),
        );
        let stdout = exec.exec(&command, Some(&self.sign_in_env())).await?;
        Ok(parse_title_for(&stdout))
    }

    /// adapter.ts:1461.
    pub async fn draft_for(&self, ask: DraftAsk, exec: Arc<dyn HarnessExec>) -> Result<Option<String>, ExecFailure> {
        let command = draft_for_command(&ask.prompt_file, ask.model.as_deref(), self.core.deps.launch.as_ref(), Some(&self.core.guard));
        let stdout = exec.exec(&command, Some(&self.sign_in_env())).await?;
        Ok(parse_draft_for(&stdout))
    }

    /// adapter.ts:1366. `Err` with the words the TS rejects with.
    pub async fn aside(&self, q: AsideQuestion) -> Result<AsideAnswer, String> {
        let core = self.core.clone();
        let wall_ms = core.deps.aside_wall_ms.unwrap_or(ASIDE_WALL_MS);
        let grace = grace_of(&core);
        let through = match &q.cut {
            Some(anchor) => CopyThrough::Anchor(anchor.clone()),
            None => CopyThrough::Live,
        };
        let copied = copy_session(self.copy_options(&q.session, through)).await.map_err(|e| e.0)?;
        let (fork, running) = (copied.session, copied.running);
        let valued = server_values_file(q.server_values.as_ref());
        let command = aside_command(&AsideCommandOptions {
            fork: fork.clone(),
            cwd: q.cwd.clone(),
            model: q.model.clone(),
            effort: q.effort.clone(),
            context_window: q.context_window.clone(),
            fast: (q.fast == Some(true)).then_some(true),
            mcp_servers: q.mcp_servers.clone(),
            server_values: valued.is_some(),
            memory_dir: core.memory_dir.clone(),
            launch: core.deps.launch.clone(),
        })
        .map_err(|e| e.0)?;
        let input = vec![aside_hooks_line(), user_message_line(&aside_prompt(&q.question, &running), &fork, &[], None)];
        let stream = core.deps.exec.launch(
            &format!("{}{}", core.guard, command),
            ExecOptions { env: core.env.clone(), input: Some(input), input_after: None, secret: valued },
        );
        let walled = Rc::new(Cell::new(false));
        let wall = {
            let (stream, walled) = (stream.clone(), walled.clone());
            tokio::task::spawn_local(async move {
                tokio::time::sleep(Duration::from_millis(wall_ms)).await;
                walled.set(true);
                fire(async move { end_run(&*stream, grace).await }.boxed_local());
            })
        };
        let mut answer: Option<Result<AsideAnswer, String>> = None;
        let mut said: Option<String> = None;
        let mut failure: Option<ExecFailure> = None;
        let mut lines = stream.lines();
        while let Some(item) = lines.next().await {
            let raw = match item {
                Ok(raw) => raw,
                Err(e) => {
                    failure = Some(e);
                    break;
                }
            };
            let Some(event) = parse_line(&raw) else {
                let text = wsp_words::js::js_trim(&raw);
                if !text.is_empty() {
                    said = Some(text.to_string());
                }
                continue;
            };
            let control = control_line(&event);
            match &control {
                Some(ControlLine::Unknown { request_id, subtype }) => fire(stream.write(if subtype == "hook_callback" {
                    hook_deny_line(request_id)
                } else {
                    control_error_line(request_id, subtype)
                })),
                Some(ControlLine::Ask { ask, .. }) => fire(stream.write(prompt_deny_line(&ask.ask_id))),
                _ => {}
            }
            if let Some(ControlLine::Answer { request_id, error: Some(error) }) = &control {
                if request_id == ASIDE_HOOKS_ID && answer.is_none() {
                    answer = Some(Err(error.clone()));
                    let stream = stream.clone();
                    fire(async move { end_run(&*stream, grace).await }.boxed_local());
                }
            }
            if let (Some(piece), Some(on_text)) = (answer.is_none().then(|| aside_text_of(&event)).flatten(), q.on_text.as_ref()) {
                on_text(piece);
            }
            if string(event.get("type")) != Some("result") || answer.is_some() || drained_notice(&event) {
                continue;
            }
            answer = Some(aside_answer(&event));
            stream.close_input();
            let stream = stream.clone();
            let exit = core.deps.result_exit_ms.unwrap_or(RUN_EXIT_MS);
            fire(async move { end_after_result(&*stream, exit, grace).await }.boxed_local());
        }
        let code = stream.exited().await;
        wall.abort();
        let _ = remove_copy(&core, &fork).await;
        if let Some(failure) = failure {
            return Err(failure.message);
        }
        match answer {
            Some(Ok(answer)) => Ok(answer),
            Some(Err(error)) => Err(error),
            None if walled.get() => Err(aside_wall_line(wall_ms)),
            None => Err(said.unwrap_or_else(|| {
                harness_exit_line(
                    "claude",
                    code,
                    core.env.get("PATH").map(String::as_str),
                    &HarnessEnded { reached: Some(false), signal: None },
                )
            })),
        }
    }

    /// adapter.ts:1357. A turn with no anchor is refused with "claude forks a session at the reply a turn ended on,
    /// and this turn named none".
    pub async fn fork_session(&self, session: &str, turn: ForkTurn) -> Result<ForkedSession, AdapterError> {
        let ForkTurn::Anchor(anchor) = turn else {
            return Err(AdapterError("claude forks a session at the reply a turn ended on, and this turn named none".into()));
        };
        let copied = copy_session(self.copy_options(session, CopyThrough::Anchor(anchor))).await?;
        let core = self.core.clone();
        let fork = copied.session.clone();
        Ok(ForkedSession::Resume {
            resume: copied.session,
            drop: Box::new(move || async move { remove_copy(&core, &fork).await.map_err(|e| e.message) }.boxed_local()),
        })
    }

    /// adapter.ts:1474.
    pub fn conversations(&self) -> Arc<dyn ConversationStore> {
        let deps = &self.core.deps;
        claude_conversations(ConversationDeps {
            config_dir: deps.config_dir.clone(),
            project_dir_name: deps.project_dir_name.clone(),
            launch: deps.launch.clone(),
            base_env: deps.base_env.clone(),
        })
    }
}

/// The runtime's view of the Claude adapter (spec/claude_adapter.md, Construction): every capability Claude Code
/// declares, and each optional TS member present.
impl crate::port::HarnessAdapter for ClaudeAdapter {
    fn start(&self, options: HarnessStartOptions) -> Result<Arc<dyn crate::port::HarnessSession>, AdapterError> {
        ClaudeAdapter::start(self, options).map(|session| Arc::new(session) as Arc<dyn crate::port::HarnessSession>)
    }
    fn warm(&self, options: HarnessWarmOptions) -> Option<Result<Box<dyn crate::port::KeptAgent>, AdapterError>> {
        Some(ClaudeAdapter::warm(self, options).map(|kept| Box::new(kept) as Box<dyn crate::port::KeptAgent>))
    }
    fn warms(&self) -> bool {
        true
    }
    fn renames(&self) -> bool {
        true
    }
    fn asides(&self) -> bool {
        true
    }
    fn attaches(&self) -> bool {
        self.core.deps.exec.attaches()
    }
    fn attach(
        &self,
        options: AdapterAttachOptions,
    ) -> Option<LocalBoxFuture<'static, Result<Option<Arc<dyn crate::port::HarnessSession>>, ExecFailure>>> {
        ClaudeAdapter::attach(self, options).map(|pending| {
            async move {
                Ok(match pending.await? {
                    AttachedSession::Session(session) => Some(Arc::new(session) as Arc<dyn crate::port::HarnessSession>),
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
    fn forks(&self) -> bool {
        true
    }
    fn fork_session(&self, session: String, turn: ForkTurn) -> Option<LocalBoxFuture<'static, Result<ForkedSession, String>>> {
        let me = self.clone();
        Some(async move { me.fork_session(&session, turn).await.map_err(|e| e.0) }.boxed_local())
    }
    fn attachments(&self) -> Option<crate::port::AttachmentRoad> {
        Some(crate::port::AttachmentRoad::Inline)
    }
    fn mcp_servers(&self) -> bool {
        true
    }
    fn denies_servers(&self) -> bool {
        true
    }
    fn screen_commands(&self) -> Vec<ScreenCommand> {
        ClaudeAdapter::screen_commands(self)
    }
    fn moves_access(&self) -> bool {
        true
    }
    fn steers_images(&self) -> bool {
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
        Some(async move { Ok(me.probe_catalog(exec).await?.map(HarnessCatalogAnswer::Probe)) }.boxed_local())
    }
    fn probe_version(&self, exec: Arc<dyn HarnessExec>) -> Option<LocalBoxFuture<'static, Result<Option<String>, ExecFailure>>> {
        let me = self.clone();
        Some(async move { me.probe_version(exec).await }.boxed_local())
    }
    fn session_title(
        &self,
        harness_session_id: String,
        exec: Arc<dyn HarnessExec>,
    ) -> Option<LocalBoxFuture<'static, Result<Option<String>, ExecFailure>>> {
        let me = self.clone();
        Some(async move { me.session_title(&harness_session_id, exec).await }.boxed_local())
    }
    fn session_file(&self, session_id: &str) -> Option<SessionFile> {
        Some(ClaudeAdapter::session_file(self, session_id))
    }
    fn rename_session(
        &self,
        harness_session_id: String,
        title: String,
        exec: Arc<dyn HarnessExec>,
    ) -> Option<LocalBoxFuture<'static, Result<SessionRenameWrite, ExecFailure>>> {
        let me = self.clone();
        Some(async move { me.rename_session(&harness_session_id, &title, exec).await }.boxed_local())
    }
    fn title_for(
        &self,
        turn: TitleTurn,
        exec: Arc<dyn HarnessExec>,
    ) -> Option<LocalBoxFuture<'static, Result<Option<String>, ExecFailure>>> {
        let me = self.clone();
        Some(async move { me.title_for(turn, exec).await }.boxed_local())
    }
    fn draft_for(&self, ask: DraftAsk, exec: Arc<dyn HarnessExec>) -> Option<LocalBoxFuture<'static, Result<Option<String>, ExecFailure>>> {
        let me = self.clone();
        Some(async move { me.draft_for(ask, exec).await }.boxed_local())
    }
    fn aside(&self, question: AsideQuestion) -> Option<LocalBoxFuture<'static, Result<AsideAnswer, String>>> {
        let me = self.clone();
        Some(async move { me.aside(question).await }.boxed_local())
    }
    fn aside_servers(&self) -> bool {
        true
    }
    fn compacts(&self) -> Option<String> {
        Some("/compact".to_string())
    }
    fn terminal_resume(&self) -> Option<String> {
        Some(ClaudeAdapter::terminal_resume(self))
    }
    fn env(&self) -> Option<Env> {
        Some(ClaudeAdapter::env(self))
    }
    fn conversations(&self) -> Option<Arc<dyn ConversationStore>> {
        Some(ClaudeAdapter::conversations(self))
    }
}

impl crate::port::Finishes for ClaudeSession {
    fn finished(&self) -> SharedFuture<TurnResult> {
        ClaudeSession::finished(self)
    }
}

/// The runtime's view of a Claude turn: every optional TS member present.
impl crate::port::HarnessSession for ClaudeSession {
    fn local_id(&self) -> String {
        ClaudeSession::local_id(self)
    }
    fn finished(&self) -> SharedFuture<TurnResult> {
        ClaudeSession::finished(self)
    }
    fn run(&self) -> Option<String> {
        ClaudeSession::run(self)
    }
    fn pid(&self) -> Option<u32> {
        ClaudeSession::pid(self)
    }
    fn from(&self) -> Option<u64> {
        ClaudeSession::from(self)
    }
    fn interrupt(&self) -> LocalBoxFuture<'static, ()> {
        let session = self.clone();
        started(async move { session.interrupt().await }.boxed_local())
    }
    fn kept(&self) -> Option<Option<Box<dyn crate::port::KeptAgent>>> {
        Some(ClaudeSession::kept(self).map(|kept| Box::new(kept) as Box<dyn crate::port::KeptAgent>))
    }
    fn steer(
        &self,
        prompt: String,
        id: Option<String>,
        images: Vec<TurnImage>,
    ) -> Option<LocalBoxFuture<'static, Result<SteerOutcome, ExecFailure>>> {
        let session = self.clone();
        Some(async move { session.steer(&prompt, id, images).await }.boxed_local())
    }
    fn tells_unread(&self) -> Option<bool> {
        Some(ClaudeSession::tells_unread(self))
    }
    fn answers(&self) -> bool {
        true
    }
    fn answer(&self, ask_id: String, answer: PermissionAnswer) -> Option<LocalBoxFuture<'static, Result<AnswerOutcome, ExecFailure>>> {
        let session = self.clone();
        Some(async move { session.answer(&ask_id, answer).await }.boxed_local())
    }
    fn set_access(&self, mode: String) -> Option<LocalBoxFuture<'static, Result<AccessOutcome, ExecFailure>>> {
        let session = self.clone();
        Some(async move { session.set_access(&mode).await }.boxed_local())
    }
    fn stop_task(&self, task: String) -> Option<LocalBoxFuture<'static, Result<TaskStop, ExecFailure>>> {
        let session = self.clone();
        Some(async move { session.stop_task(&task).await }.boxed_local())
    }
}

impl crate::port::KeptAgent for ClaudeKept {
    fn next(&self, turn: KeptTurn) -> Result<Arc<dyn crate::port::HarnessSession>, crate::port::AdapterError> {
        Ok(Arc::new(ClaudeKept::next(self, turn)))
    }
    fn close(&self, now: bool) -> LocalBoxFuture<'static, ()> {
        let kept = self.clone();
        started(async move { kept.close(now).await }.boxed_local())
    }
    fn exited(&self) -> SharedFuture<()> {
        ClaudeKept::exited(self).map(|_| ()).boxed_local().shared()
    }
    fn session_file(&self) -> Option<SessionFile> {
        ClaudeKept::session_file(self)
    }
}
