// SPDX-License-Identifier: AGPL-3.0-only
//! sessions.ts:255-475, 1486-1496: the checks a start makes before it holds the thread's row.
//! Spec: spec/sessions_start.md.
use std::rc::Rc;
use std::sync::Arc;

use futures::FutureExt;
use indexmap::{IndexMap, IndexSet};
use wsp_account::account::rules::TreeOf;
use wsp_adapters::port::ForkTurn;
use wsp_catalog::signin_rows::server_values_of;
use wsp_core::console;
use wsp_core::machine_exec::TurnWaiting;
use wsp_core::types::api::{ForkOptions, StartOptions};
use wsp_core::types::internal::TreeTalk;
use wsp_core::types::wiring::{SessionHandle, WorkspaceRecord};
use wsp_machines::machines::boot::entries::RowAt;
use wsp_wire::types::attachments::AttachmentRecord;
use wsp_wire::types::conversations::{RESUME_A_THREAD_FIX, RESUME_A_THREAD_LINE};
use wsp_wire::types::env::{THREAD_ENV, TURN_TOKEN_ENV};
use wsp_wire::types::wsp_tools::MCP_SERVER_NAME;
use wsp_wire::views::harness::{mcp_servers_blocked, McpServerSpec};
use wsp_wire::views::preferences::Preferences;
use wsp_wire::views::session_events::NOTIFY_ME;
use wsp_wire::views::workspace::{road_of, scope_of, Caller, WorkspacePhase};
use wsp_wire::views::workspace_more::{ThreadScope, WorkspaceOrigin};
use wsp_words::attachments::{attachment_record, files_blocked};
use wsp_words::exit::{refusal, HostError, Kind};
use wsp_words::place_state::deleted_before_start_line;
use wsp_words::projects::runs_in_folder;
use wsp_words::sub_thread::{sub_machine_taken_refusal, sub_opening};
use wsp_words::thread_defaults::agent_off_line;
use wsp_words::words::computer::{
    send_files_across_line, thread_word, wait_across_line, RESTART_OPENS_FIX, RESTART_OPENS_LINE, SEND_FILES_ACROSS_FIX, WAIT_ACROSS_FIX,
};
use wsp_words::words::thread::{
    fork_refused_line, forks_not_line, thread_runs_on_line, ASIDE_NO_SESSION_LINE, EMPTY_TITLE_LINE, FORKS_NOT_FIX, FORK_REFUSED_FIX,
};
use wsp_words::words::turn::title_line;
use wsp_words::workspace_state::{send_refusal, workspace_state, SendRefusalKind, WorkspaceStateInput};

use super::{ForkPlan, SessionsArea, SessionsCtx};
use crate::threads::agents::launch::{AdapterAsk, BuiltAdapter};
use crate::threads::launch::{clouds_told, served_to, LaunchCtx, ThreadLaunch};
use crate::threads::threads::{Entry, Waiting};

/// A start that passed every check, carried to its launch (sessions_launch).
pub struct Prepared {
    pub workspace_id: String,
    pub origin: Option<Caller>,
    /// The options with the harness settled (TS `o`), the title trimmed in.
    pub opts: StartOptions,
    pub prefs: Preferences,
    pub plan: Option<ForkPlan>,
    pub thread_id: String,
    pub opens: bool,
    pub entry: Entry,
    pub replaces: Option<String>,
    pub resume: Option<String>,
    /// The worktree path found removed by hand, where one was.
    pub gone_now: Option<String>,
    pub title: Option<String>,
    pub tree: TreeOf,
    /// The notify targets this start named (TS `asked`).
    pub asked: Option<Vec<String>>,
    pub notify: Option<Vec<String>>,
    pub notify_by: Option<ThreadScope>,
    pub notify_road: Option<WorkspaceOrigin>,
    pub notice: Option<String>,
    pub clouds: Option<Vec<String>>,
    /// Whether the thread ran no turn when the checks ran (TS `cleared`).
    pub cleared: bool,
    pub turn_token: String,
    pub launch: ThreadLaunch,
    pub built: BuiltAdapter,
    pub fork_turn: Option<ForkTurn>,
    pub records: Vec<AttachmentRecord>,
    pub mcp_servers: Option<indexmap::IndexMap<String, McpServerSpec>>,
    pub waiting: Waiting,
}

/// What the checks came to: an answer already, or a start to launch.
pub enum Checked {
    Answered(Rc<dyn SessionHandle>),
    Launch(Box<Prepared>),
}

fn record_of(entry: &Entry) -> WorkspaceRecord {
    entry.borrow().record.borrow().clone()
}

/// sessions.ts:1494: a refusal of a send to a workspace in its state, and the sub-machine's own refusal first.
pub(crate) fn refuse_now(ctx: &Rc<dyn SessionsCtx>, workspace_id: &str, entry: &Entry) -> Result<(), HostError> {
    let Some(live) = ctx.live_entry(workspace_id) else {
        return Err(HostError::new(deleted_before_start_line(&record_of(entry).name)));
    };
    ctx.refuse_sub_send(&live)?;
    if live.borrow().record.borrow().phase == WorkspacePhase::Released {
        return Ok(());
    }
    let record = record_of(entry);
    let state = workspace_state(&WorkspaceStateInput { phase: record.phase, machine_state: None, reach: None, wake_refused: None });
    match send_refusal(&SendRefusalKind::State(state), record.gone.as_deref(), Some(record.name.as_str())) {
        Some(line) => Err(HostError::new(line)),
        None => Ok(()),
    }
}

/// The revoke of a scoped device, detached (TS `dropScope`): a failure is warned, never raised.
pub(crate) fn drop_scope(ctx: &Rc<dyn SessionsCtx>, device: Option<&str>, thread_id: &str) {
    let Some(device) = device.map(str::to_string) else { return };
    let revoker = ctx.clone();
    let thread = thread_word(thread_id);
    ctx.spawn(Box::pin(async move {
        if let Err(e) = revoker.revoke_device(&device).await {
            console::warn(&format!("the token of thread {thread} was not taken away: {}", e.message));
        }
    }));
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// The turn env a launch carries: the turn's token and thread, then the launch's own (TS `{ [TURN_TOKEN_ENV]: ... }`).
fn turn_env(turn_token: &str, thread_id: &str, launch_env: &IndexMap<String, String>) -> wsp_adapters::port::Env {
    let mut env = wsp_adapters::port::Env::new();
    env.insert(TURN_TOKEN_ENV.to_string(), turn_token.to_string());
    env.insert(THREAD_ENV.to_string(), thread_id.to_string());
    for (name, value) in launch_env {
        env.insert(name.clone(), value.clone());
    }
    env
}

fn waiting_probe(waiting: &Waiting) -> TurnWaiting {
    let waiting = waiting.clone();
    Arc::new(move || waiting.get())
}

impl SessionsArea {
    /// sessions.ts:1489, the wrapper over the whole start. Rule S26; the rest is `prepare` then `launch_prepared`.
    pub async fn start_send(
        &self,
        workspace_id: &str,
        opts: StartOptions,
        origin: Option<Caller>,
    ) -> Result<Rc<dyn SessionHandle>, HostError> {
        let claimed = std::cell::RefCell::new(None);
        let outcome = match self.prepare(workspace_id, opts, origin, &claimed).await {
            Ok(Checked::Answered(handle)) => Ok(handle),
            Ok(Checked::Launch(prepared)) => self.launch_prepared(*prepared).await,
            Err(e) => Err(e),
        };
        if outcome.is_err() {
            let warm = claimed.borrow().clone();
            if let Some(thread) = warm.filter(|thread| self.ctx.kept_warm(thread)) {
                <dyn SessionsCtx as LaunchCtx>::reap_kept(&*self.ctx, &thread);
            }
            if self.ctx.live_entry(workspace_id).is_some_and(|live| live.borrow().record.borrow().sub.is_some()) {
                self.ctx.sub_done(workspace_id);
            }
        }
        outcome
    }

    /// sessions.ts:255-475. `claimed` takes the warm agent's thread where the start claimed one. Rules S1-S25.
    pub async fn prepare(
        &self,
        workspace_id: &str,
        opts: StartOptions,
        origin: Option<Caller>,
        claimed: &std::cell::RefCell<Option<String>>,
    ) -> Result<Checked, HostError> {
        let ctx = self.ctx.clone();
        ctx.ready().await?;
        let prefs = match ctx.preferences_held() {
            Some(prefs) => prefs,
            None => ctx.preferences().await?,
        };
        if let Some(request_id) = opts.request_id.as_deref() {
            if let Some(taken) = ctx.taken(workspace_id, request_id) {
                let at = RowAt { thread_id: Some(taken.thread_id.clone()), workspace_id: workspace_id.to_string() };
                if ctx.entry_of_row(&at, origin.as_ref(), Some(TreeTalk::Send), false).await?.is_some() {
                    if let Some(answered) = ctx.taken_turn(workspace_id, taken).await? {
                        return Ok(Checked::Answered(answered));
                    }
                }
            }
        }
        let (mut opened, plan) = ctx.fork_opened(workspace_id, opts, origin.as_ref()).await?;
        let named = opened.thread.clone().and_then(|thread| ctx.latest_on(&thread));
        let from_transcript = opened.thread.clone().and_then(|thread| ctx.started_as(workspace_id, &thread));
        let held_on = match opened.thread.clone() {
            None => None,
            Some(thread) => named
                .as_ref()
                .map(|view| view.workspace_id.clone())
                .or_else(|| ctx.records().borrow().get(&thread).map(|record| record.workspace_id.clone()))
                .or_else(|| from_transcript.as_ref().map(|_| workspace_id.to_string())),
        };
        if let Some(thread) = opened.thread.as_deref() {
            if held_on.as_deref() != Some(workspace_id) {
                return Err(HostError::new(format!("no thread {thread} in this folder")));
            }
        }
        let outside = opened.outside.clone();
        if outside.is_some() && opened.thread.is_some() {
            return Err(refusal(RESUME_A_THREAD_LINE, RESUME_A_THREAD_FIX, Some(Kind::Usage)));
        }
        let resume = named
            .as_ref()
            .and_then(|view| view.claude_session_id.clone())
            .or(from_transcript)
            .or_else(|| outside.as_ref().filter(|o| !o.copy).map(|o| o.id.clone()));
        let warmed = match opened.harness.clone() {
            Some(harness)
                if opened.thread.is_none()
                    && plan.is_none()
                    && outside.is_none()
                    && scope_of(origin.as_ref()).is_none()
                    && opened.replaces.is_none()
                    && opened.title.is_none()
                    && opened.mcp_servers.is_none()
                    && !opened.asks_until_stopped =>
            {
                ctx.claim_warm(workspace_id, &harness)
            }
            _ => None,
        };
        if let Some(warm) = &warmed {
            *claimed.borrow_mut() = Some(warm.clone());
        }
        let thread_id = match opened.thread.clone().or_else(|| opened.r#as.as_ref().map(|a| a.thread_id.clone())).or_else(|| warmed.clone())
        {
            Some(thread) => thread,
            None => {
                let opens_thread = ctx.live_entry(workspace_id).and_then(|live| live.borrow().opens_thread.clone());
                match opens_thread.filter(|made| !ctx.records().borrow().contains_key(made) && ctx.rows_on(made).is_empty()) {
                    Some(made) => made,
                    None => ctx.ids().uuid(),
                }
            }
        };
        let opens = !ctx.records().borrow().contains_key(&thread_id) && ctx.rows_on(&thread_id).is_empty();
        let opening: Option<WorkspaceRecord> = ctx.live_entry(workspace_id).map(|live| live.borrow().record.borrow().clone());
        if let Some(record) = opening.as_ref().filter(|record| opens && record.lead.is_some()) {
            let sub_thread = ctx.records().borrow().iter().find(|(_, r)| r.workspace_id == workspace_id).map(|(id, _)| id.clone());
            if let Some(sub_thread) = sub_thread {
                return Err(sub_machine_taken_refusal(&record.name, &sub_thread));
            }
        }
        let opens_there = match opening.as_ref() {
            Some(record) => ctx.opens_in(record, origin.as_ref()),
            None => false,
        };
        if let Some(record) = opening.as_ref() {
            if opens && !opens_there && ctx.of_threads_repository(origin.as_ref(), &record.project) {
                let away = if runs_in_folder(&record.kind) {
                    ctx.elsewhere_refusal(origin.as_ref(), &record.project, &record.name)
                } else {
                    ctx.away_for(record, origin.as_ref(), "start", workspace_id)
                };
                if let Some(away) = away {
                    return Err(away);
                }
            }
        }
        let entry = if opens {
            ctx.entry_of(workspace_id, if opens_there { None } else { origin.clone() }).await?
        } else {
            let at = RowAt { thread_id: Some(thread_id.clone()), workspace_id: workspace_id.to_string() };
            match ctx.entry_of_row(&at, origin.as_ref(), Some(TreeTalk::Send), false).await? {
                Some(entry) => entry,
                None => {
                    return Err(HostError::new(format!("no thread {} in this folder", opened.thread.as_deref().unwrap_or("undefined"))))
                }
            }
        };
        let replaces = opened.replaces.clone();
        if let Some(replaces) = replaces.as_deref() {
            if !opens {
                return Err(refusal(RESTART_OPENS_LINE, RESTART_OPENS_FIX, Some(Kind::Usage)));
            }
            self.replaceable_check(replaces, origin.as_ref()).await?;
        }
        let asking: Option<WorkspaceRecord> = match scope_of(origin.as_ref()) {
            None => None,
            Some(_) if opens || ctx.acts_on(&record_of(&entry), origin.as_ref()) => None,
            Some(scope) => ctx.live_entry(&scope.workspace_id).map(|live| live.borrow().record.borrow().clone()),
        };
        if let Some(asking) = asking {
            let to_record = record_of(&entry);
            let from = ctx.project_computer_name(&asking.project);
            let to = ctx.project_computer_name(&to_record.project);
            if opened.attachments.as_ref().is_some_and(|files| !files.is_empty()) {
                return Err(refusal(&send_files_across_line(&from, &to, &thread_id), SEND_FILES_ACROSS_FIX, Some(Kind::Usage)));
            }
            if opened.followed == Some(true) {
                return Err(refusal(&wait_across_line(&from, &to, &thread_id), WAIT_ACROSS_FIX, Some(Kind::Usage)));
            }
        }
        let gone_now = {
            let record = record_of(&entry);
            match record.worktree.as_ref() {
                Some(worktree) if worktree.gone.is_none() && !ctx.folder_exists(&worktree.path) => Some(worktree.path.clone()),
                _ => None,
            }
        };
        if gone_now.is_some() {
            ctx.worktree_gone(&entry, "removed", true).await?;
        }
        let carried: Option<String> = if opens {
            None
        } else {
            let held = ctx.records().borrow().get(&thread_id).map(|record| record.harness.clone());
            held.or_else(|| ctx.latest_on(&thread_id).map(|view| view.harness))
        };
        if let (Some(carried), Some(asked)) = (carried.as_deref(), opened.harness.as_deref()) {
            if carried != asked {
                return Err(HostError::new(thread_runs_on_line(carried, asked)));
            }
        }
        if outside.is_some() && !opens {
            return Err(refusal(RESUME_A_THREAD_LINE, RESUME_A_THREAD_FIX, Some(Kind::Usage)));
        }
        let harness = match carried.clone().or_else(|| outside.as_ref().map(|o| o.harness.clone())).or_else(|| opened.harness.clone()) {
            Some(harness) => harness,
            None => ctx.default_agent_of(&prefs, &entry),
        };
        opened.harness = Some(harness.clone());
        if carried.is_some() {
            opened.permission_mode = None;
            opened.access = None;
        }
        if ctx.agent_off(&entry, &harness) {
            let line = agent_off_line(&ctx.agent_label(&harness), &ctx.computer_of(&entry));
            return Err(HostError::with_kind(line, Kind::Usage));
        }
        ctx.confine_setup(&entry, &harness).await?;
        if entry.borrow().unchecked && record_of(&entry).phase == WorkspacePhase::Running {
            ctx.proven(&entry).await?;
        }
        refuse_now(&ctx, workspace_id, &entry)?;
        let cleared = !ctx.thread_runs(&thread_id);
        if cleared {
            ctx.copy_blocked(&entry).await?;
            ctx.worktree_mounted(&entry).await?;
            ctx.kept_check(&thread_id).await?;
        }
        let title = opened.title.as_deref().map(title_line);
        if title.as_deref() == Some("") {
            return Err(HostError::new(EMPTY_TITLE_LINE));
        }
        opened.title = title.clone();
        ctx.spawn_guard(if opens { "thread_new" } else { "send" }, origin.as_ref())?;
        let spawned_by = if opens { scope_of(origin.as_ref()) } else { None };
        let tree: TreeOf = if opens {
            match replaces.as_deref().and_then(|r| ctx.parent_of(r).map(|parent| (parent, ctx.root_of(r)))) {
                Some((parent, root)) if spawned_by.is_none() => TreeOf { parent_thread_id: Some(parent), root_thread_id: Some(root) },
                _ => ctx.tree_of(spawned_by.as_ref()),
            }
        } else {
            let root = ctx.root_of(&thread_id);
            TreeOf { parent_thread_id: ctx.parent_of(&thread_id), root_thread_id: if root != thread_id { Some(root) } else { None } }
        };
        let asked: Option<Vec<String>> = match opened.notify.clone() {
            None => None,
            Some(targets) => {
                let mut resolved: IndexSet<String> = IndexSet::new();
                for target in targets {
                    let target = match opened.turn_token.as_deref() {
                        Some(token) if target == NOTIFY_ME => ctx.thread_of_token(token)?,
                        _ => target,
                    };
                    resolved.insert(target);
                }
                Some(resolved.into_iter().collect())
            }
        };
        for target in asked.iter().flatten().filter(|target| *target != NOTIFY_ME) {
            match ctx.latest_on(target) {
                Some(view) if ctx.reaches_row(&view, origin.as_ref(), None) => {}
                _ => return Err(HostError::new(format!("no thread {target} to notify"))),
            }
            if *target == thread_id {
                return Err(HostError::new("a thread cannot notify itself"));
            }
            let target_notify = ctx.notify_on(target).map(|on| on.notify).unwrap_or_default();
            if ctx.notify_reach(&target_notify).contains(&thread_id) {
                let short: String = target.chars().take(8).collect();
                return Err(HostError::new(format!("thread {short} already notifies this thread; a cycle would run forever")));
            }
        }
        let registered = ctx.notify_on(&thread_id);
        let notify = asked.clone().or_else(|| registered.as_ref().map(|r| r.notify.clone()));
        let notify_by = if asked.is_none() { registered.as_ref().and_then(|r| r.by.clone()) } else { scope_of(origin.as_ref()) };
        let notify_road = if asked.is_none() { registered.as_ref().and_then(|r| r.road) } else { road_of(origin.as_ref()) };
        let mut entry = entry;
        let mut notice: Option<String> = None;
        if entry.borrow().record.borrow().sub.is_some() {
            let resumed = ctx.resume_sub(&entry, &thread_id, &harness, &opened.prompt).await?;
            entry = resumed.entry;
            if let Some(preamble) = resumed.preamble.as_deref() {
                opened.prompt = sub_opening(preamble, &opened.prompt);
            }
            if !resumed.notices.is_empty() {
                notice = Some(resumed.notices.join("\n"));
            }
        }
        let clouds = if runs_in_folder(&record_of(&entry).kind) && record_of(&entry).sub.is_none() {
            let rows = ctx.place_rows().await;
            let held = clouds_told(rows, &ctx.records(), &thread_id, &mut opened.prompt);
            Some(held)
        } else {
            None
        };
        if record_of(&entry).queue.is_some() {
            let handle = ctx.hold_queued(workspace_id, &thread_id, tree, opened.clone(), asked, origin.as_ref()).await?;
            return Ok(Checked::Answered(handle));
        }
        let turn_token = hex(&ctx.ids().bytes(16));
        let root = tree.root_thread_id.clone().unwrap_or_else(|| thread_id.clone());
        let launch = ctx.thread_launch(&entry, &thread_id, &root, false).await?;
        let waiting: Waiting = Rc::new(std::cell::Cell::new(false));
        let built = match ctx.adapter_for(
            &entry,
            AdapterAsk {
                named: Some(harness.clone()),
                turn_env: Some(turn_env(&turn_token, &thread_id, &launch.env)),
                waiting: Some(waiting_probe(&waiting)),
                servers: server_values_of(&ctx.vault()),
                thread: Some(thread_id.clone()),
                asks_until_stopped: opened.asks_until_stopped,
            },
        ) {
            Ok(built) => built,
            Err(e) => {
                drop_scope(&ctx, launch.scoped_device_id.as_deref(), &thread_id);
                return Err(e);
            }
        };
        let adapter = built.adapter.clone();
        let no_fork_road = !ctx.adapter_forks(adapter.as_ref());
        let fork_turn = match plan.as_ref() {
            None => None,
            Some(plan) => {
                let turn_check = plan.turn.is_some() && (no_fork_road || plan.source.session.is_none());
                if turn_check {
                    drop_scope(&ctx, launch.scoped_device_id.as_deref(), &thread_id);
                    let label = ctx.agent_label(&harness);
                    return Err(if no_fork_road {
                        refusal(&forks_not_line(&label), FORKS_NOT_FIX, Some(Kind::Usage))
                    } else {
                        refusal(&fork_refused_line(&label, ASIDE_NO_SESSION_LINE), FORK_REFUSED_FIX, Some(Kind::Conflict))
                    });
                }
                match ctx.fork_turn_of(plan, adapter.forks_by_count()) {
                    Ok(turn) => turn,
                    Err(e) => {
                        drop_scope(&ctx, launch.scoped_device_id.as_deref(), &thread_id);
                        return Err(e);
                    }
                }
            }
        };
        let served = served_to(launch.wsp.as_ref(), &root, &thread_id);
        let mcp_servers = match served {
            Some(served) if adapter.mcp_servers() => {
                let mut servers: IndexMap<String, McpServerSpec> = IndexMap::new();
                servers.insert(MCP_SERVER_NAME.to_string(), served);
                for (name, spec) in opened.mcp_servers.clone().unwrap_or_default() {
                    servers.insert(name, spec);
                }
                Some(servers)
            }
            _ => opened.mcp_servers.clone(),
        };
        let records: Vec<AttachmentRecord> = opened.attachments.clone().unwrap_or_default().iter().map(attachment_record).collect();
        let road = adapter.attachments().map(|road| match road {
            wsp_adapters::port::AttachmentRoad::Inline => wsp_words::attachments::AttachmentRoad::Inline,
            wsp_adapters::port::AttachmentRoad::File => wsp_words::attachments::AttachmentRoad::File,
        });
        let blocked = files_blocked(&records, road.as_ref(), &harness)
            .or_else(|| mcp_servers_blocked(opened.mcp_servers.as_ref(), adapter.mcp_servers(), &harness));
        if let Some(blocked) = blocked {
            drop_scope(&ctx, launch.scoped_device_id.as_deref(), &thread_id);
            return Err(HostError::new(blocked));
        }
        Ok(Checked::Launch(Box::new(Prepared {
            workspace_id: workspace_id.to_string(),
            origin,
            opts: opened,
            prefs,
            plan,
            thread_id,
            opens,
            entry,
            replaces,
            resume,
            gone_now,
            title,
            tree,
            asked,
            notify,
            notify_by,
            notify_road,
            notice,
            clouds,
            cleared,
            turn_token,
            launch,
            built,
            fork_turn,
            records,
            mcp_servers,
            waiting,
        })))
    }

    /// sessions.ts:897.
    pub async fn fork_send(&self, opts: ForkOptions, origin: Option<Caller>) -> Result<Rc<dyn SessionHandle>, HostError> {
        let area = self.clone();
        let call_origin = origin.clone();
        let start: crate::threads::forks::StartFn = Rc::new(move |workspace_id: String, opts: StartOptions| {
            let area = area.clone();
            let origin = call_origin.clone();
            async move { area.start_send(&workspace_id, opts, origin).await }.boxed_local()
        });
        self.ctx.fork_start(opts, start, origin.as_ref()).await
    }
}
