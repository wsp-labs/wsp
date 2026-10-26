// SPDX-License-Identifier: AGPL-3.0-only
//! Sessions, images, usage, forwards, folders, agents, editors, the host's own doors and the project import and export.
//! serve.ts:333-347, 419-440, 857-862, 1474-1633, 1643-1655, 1679-1895. Spec: spec/serve_ops.md.
use std::rc::Rc;

use async_trait::async_trait;
use futures::FutureExt;
use serde::Serialize;
use serde_json::{json, Map, Value};

use wsp_core::types::api::{
    AnswerAsk, HistoryAsk, ImageBuildAsk, KeyWay, PageWindow, PrepareAsk, RewindAsk, SkillsAddAsk, SteerAsk, ToolsAsk, WarmAsk,
    WorkspacesApi,
};
use wsp_core::types::pending::{SignInAsk, SkillAsk};
use wsp_core::types::wiring::{
    EditorOpen, EditorRemote, FolderListAsk, ProjectBundler, ProjectExportOptions, ProjectImportOptions, ProjectLander, ReadingsTarget,
};
use wsp_machines::places::types::NO_PLACE_DOOR;
use wsp_threads::threads::answered_start::{start_at, StartAtMsg, StartAtWorkspaces};
use wsp_wire::types::agents_report::{PluginAsk, ServerAdd, ServerAsk, AGENTS_KEY_REFUSAL, SIGN_IN_LINE_REFUSAL};
use wsp_wire::types::check::parse;
use wsp_wire::types::place_word::HERE_PLACE_ID;
use wsp_wire::types::release::{HOST_NO_RESTART_LINE, HOST_RESTART_TICKET_REFUSAL};
use wsp_wire::types::thread_defaults::{AgentSetupSet, EnvName, ENV_VALUE_REFUSAL};
use wsp_wire::views::preferences::{
    editor_opens_here_line, ssh_copy_here_line, ssh_include_line, EDITOR_TICKET_REFUSAL, SSH_TICKET_REFUSAL,
};
use wsp_wire::views::project_bundle::HostFolderListing;
use wsp_wire::views::workspace::Caller;
use wsp_wire::views::workspace::WorkspacePhase;
use wsp_wire::views::workspace_more::{WorkspaceOrigin, WorkspaceView};
use wsp_wire::wire::daemon_version::{place_behind_line, place_daemon_behind, FS_FOLDERS_DAEMON_VERSION};
use wsp_wire::wire::machine_link::{is_joined_computer, no_such_place_refusal};
use wsp_wire::wire::place_link::PLACES_TICKET_REFUSAL;
use wsp_wire::wire::runtime_ops::{RuntimeOp, RuntimeRequest};
use wsp_words::exit::{usage_refusal, HostError, Kind};
use wsp_words::place_name::table_name;
use wsp_words::place_word::PlaceRef;
use wsp_words::project_path::ssh_alias;
use wsp_words::words::units::provider_folders_refusal;
use wsp_words::workspace_state::{absent_computer, is_local_workspace};

use super::answer::{ok_reply, refusal_reply, Handled};
use super::door::Served;
use super::places::refused_off_own_road;
use super::served::DynRuntime;
use super::served::Payload;
use super::socket::Socket;
use super::{NO_BUNDLER, NO_EDITOR, NO_FOLDERS, NO_INIT, NO_LANDER, NO_RELEASE, NO_SSH, NO_TERMINAL_CONFIG};
use crate::serve_start::{serve_start, AnswerFn};

/// What `sessions.history` refuses a window with no thread with: the sentence and the fix. serve.ts:1495.
pub const HISTORY_WINDOW_LINE: &str = "before and limit page one thread's events.";
pub const HISTORY_WINDOW_FIX: &str = "Name the thread with threadId.";
/// What `places.readings` naming neither target is refused with, kind usage. serve.ts:1597.
pub const READINGS_TARGET_REFUSAL: &str = "places.readings names a placeId or a workspaceId";
/// The default image name `snapshots.*` read. serve.ts:1569, 1610.
pub const DEFAULT_IMAGE: &str = "default";

/// Answers the ops of this unit and sends their replies; `NotMine` for any other op.
pub async fn answer_ops(
    socket: &Rc<Socket>,
    frame: &serde_json::Value,
    msg: &RuntimeRequest,
    origin: Option<Caller>,
) -> Result<Handled, HostError> {
    let served = socket.served.clone();
    let rt: &DynRuntime = &*served.rt;
    let road = socket.road();
    let id = serde_json::to_value(msg.op.id()).unwrap_or(Value::Null);
    let fields = match &msg.op {
        RuntimeOp::SessionsStart { .. } => {
            let answering = socket.clone();
            let (answer_id, answer_frame) = (id.clone(), frame.clone());
            let answer: AnswerFn = Rc::new(move |result: wsp_wire::wire::runtime::SessionStartResult| {
                answering.answer(Some(&answer_frame), ok_reply(&answer_id, spread_or_empty(&result)), None)
            });
            serve_start(rt, msg, origin, answer).await?;
            return Ok(Handled::Answered);
        }
        RuntimeOp::SessionsWarm { workspace_id, project, cwd, harness, model, effort, permission_mode, access, fast, .. } => {
            let picks = StartAtMsg {
                workspace_id: workspace_id.clone(),
                project: project.clone(),
                branch: None,
                cwd: cwd.clone(),
                harness: harness.clone(),
                model: model.clone(),
                effort: effort.clone(),
                access: *access,
                permission_mode: permission_mode.clone(),
                fast: *fast,
            };
            let at = start_at(&WorkspacesAt(rt.workspaces()), picks, origin.clone(), false).await?;
            let ask = WarmAsk {
                harness: harness.clone(),
                cwd: at.cwd,
                model: model.clone(),
                effort: effort.clone(),
                permission_mode: permission_mode.clone(),
                access: *access,
                fast: *fast,
                ..Default::default()
            };
            spread(&rt.sessions().warm(&at.workspace_id, ask, origin).await?)?
        }
        RuntimeOp::HarnessesList { workspace_id, project, computer, .. } => {
            let at = wsp_core::types::api::CatalogAt { project: project.clone(), computer: computer.clone() };
            one("harnesses", to_value(&rt.harnesses().list(workspace_id.clone(), origin, Some(at)).await?)?)
        }
        RuntimeOp::SessionsList { workspace_id, .. } => {
            one("sessions", to_value(&rt.sessions().list(workspace_id.clone(), origin).await?)?)
        }
        RuntimeOp::SessionsHistory { workspace_id, thread_id, before, limit, whole, .. } => {
            let whole = whole.is_some();
            let (before, limit) = (before.and_then(unsigned_int), limit.and_then(unsigned_int));
            match thread_id {
                Some(thread_id) if !whole => {
                    let window = PageWindow { thread_id: thread_id.clone(), before, limit };
                    spread(&rt.sessions().page(workspace_id, window, origin).await?)?
                }
                _ if !whole && (before.is_some() || limit.is_some()) => {
                    return Err(usage_refusal(HISTORY_WINDOW_LINE, HISTORY_WINDOW_FIX));
                }
                _ if whole => {
                    let asked = HistoryAsk { thread_id: thread_id.clone(), before, limit };
                    one("events", to_value(&rt.sessions().thread_history(workspace_id, asked, origin).await?)?)
                }
                _ => one("events", to_value(&rt.sessions().history(workspace_id, origin).await?)?),
            }
        }
        RuntimeOp::SessionsHead { thread_id, after_read, .. } => {
            let after_read = after_read.as_ref().map(|_| true);
            spread(&rt.sessions().head(thread_id, origin, after_read).await?)?
        }
        RuntimeOp::SessionsAttachment { workspace_id, thread_id, request_id, index, .. } => {
            let index = u32::try_from((*index).get()).unwrap_or_default();
            let attachment = rt.sessions().attachment(workspace_id, thread_id, request_id, index, origin).await?;
            one("attachment", to_value(&attachment)?)
        }
        RuntimeOp::SessionsInterrupt { session_id, task, .. } => spread(&rt.sessions().interrupt(session_id, origin, task.clone()).await?)?,
        RuntimeOp::SessionsAnswer { session_id, ask_id, option_id, reason, .. } => {
            let ask = AnswerAsk { ask_id: ask_id.clone(), option_id: option_id.clone(), reason: reason.clone() };
            spread(&rt.sessions().answer(session_id, ask, origin).await?)?
        }
        RuntimeOp::SessionsAccess { session_id, permission_mode, .. } => {
            spread(&rt.sessions().access(session_id, permission_mode, origin).await?)?
        }
        RuntimeOp::SessionsRename { session_id, title, .. } => spread(&rt.sessions().rename(session_id, title, origin).await?)?,
        RuntimeOp::SessionsForget { thread_id, check, .. } => {
            rt.sessions().forget(thread_id, origin, check.filter(|check| *check)).await?;
            Payload::new()
        }
        RuntimeOp::SessionsRead { thread_id, .. } => {
            rt.sessions().read(thread_id, origin).await?;
            Payload::new()
        }
        RuntimeOp::SessionsReplaceable { thread_id, .. } => {
            rt.sessions().replaceable(thread_id, origin).await?;
            Payload::new()
        }
        RuntimeOp::SessionsMark { thread_ids, marks, .. } => {
            rt.sessions().mark(thread_ids.clone(), marks.clone(), origin).await?;
            Payload::new()
        }
        RuntimeOp::SessionsRestore { thread_ids, .. } => spread(&rt.sessions().restore(thread_ids.clone(), origin).await?)?,
        RuntimeOp::SessionsSearch { query, .. } => spread(&rt.sessions().search(query, origin).await?)?,
        RuntimeOp::SessionsAside { session_id, question, ask_id, .. } => {
            spread(&rt.sessions().aside(session_id, question, origin, ask_id.clone()).await?)?
        }
        RuntimeOp::SessionsSteer { session_id, prompt, request_id, attachments, .. } => {
            let ask = SteerAsk { prompt: prompt.clone(), request_id: request_id.clone(), attachments: attachments.clone() };
            spread(&rt.sessions().steer(session_id, ask, origin).await?)?
        }
        RuntimeOp::SessionsRewind { thread_id, turn_id, files, undo, .. } => {
            let ask = RewindAsk { turn_id: turn_id.clone(), files: *files, undo: *undo };
            spread(&rt.sessions().rewind(thread_id, ask, origin).await?)?
        }
        RuntimeOp::SessionsRun { run_id, block, command, state, pty_id, exit_code, signal, output, thread_id, turn_id, .. } => {
            let step = wsp_wire::views::session_events_more::RunStep {
                run_id: run_id.clone(),
                block: block.clone(),
                command: command.clone(),
                state: *state,
                pty_id: pty_id.clone(),
                exit_code: *exit_code,
                signal: *signal,
                output: output.clone(),
                thread_id: thread_id.clone(),
                turn_id: turn_id.clone(),
            };
            one("run", to_value(&rt.sessions().run(step, origin).await?)?)
        }
        RuntimeOp::GoldenGet { name, .. } => one("manifest", to_value(&rt.golden().get(Some(name.clone())).await?)?),
        RuntimeOp::CapabilitiesGet { .. } => {
            let capabilities = rt.backend().capabilities();
            one("capabilities", to_value(&capabilities)?)
        }
        RuntimeOp::GoldenPrepare { name, kind, .. } => {
            let ask = PrepareAsk { name: Some(name.clone()), kind: *kind, ..Default::default() };
            one("builder", to_value(&rt.golden().prepare(ask).await?)?)
        }
        RuntimeOp::GoldenSeal { builder_id, .. } => {
            let sealed = rt.golden().seal(builder_id, None, None).await?;
            let mut out = one("manifest", to_value(&sealed.manifest)?);
            out.insert("version".into(), to_value(&sealed.version)?);
            out
        }
        RuntimeOp::SnapshotsList { name, .. } => {
            let name = name.clone().unwrap_or_else(|| DEFAULT_IMAGE.to_string());
            let manifest = rt.golden().get(Some(name.clone())).await?;
            let (head, versions) = match &manifest {
                Some(manifest) => (to_value(&manifest.head)?, to_value(&manifest.versions)?),
                None => (Value::Null, json!([])),
            };
            one("lineage", json!({ "name": name, "head": head, "versions": versions }))
        }
        RuntimeOp::SnapshotsStorage { .. } => one("storage", to_value(&rt.golden().storage().await?)?),
        RuntimeOp::SnapshotsRollback { version, name, .. } => {
            let name = name.clone().unwrap_or_else(|| DEFAULT_IMAGE.to_string());
            let manifest = rt.golden().rollback(version.0 as u32, Some(name.clone())).await?;
            let lineage = json!({ "name": name, "head": to_value(&manifest.head)?, "versions": to_value(&manifest.versions)? });
            let mut out = one("lineage", lineage);
            out.insert("existingWorkspaces".into(), json!("untouched"));
            out
        }
        RuntimeOp::ImageGet { name, .. } => one("view", to_value(&rt.image().get(name.clone()).await?)?),
        RuntimeOp::ImageBuild { place, name, force, .. } => {
            let ask = ImageBuildAsk { place: place.clone(), name: name.clone(), force: *force, ..Default::default() };
            one("build", to_value(&rt.image().build(ask).await?)?)
        }
        RuntimeOp::CostHistory { workspace_id, .. } => {
            let status = rt.status();
            one("points", to_value(&wsp_core::types::wiring::OriginStatusApi::history(&*status, workspace_id, origin).await?)?)
        }
        RuntimeOp::CostSpend { .. } => {
            let rows = if road.by.is_some() {
                crate::serve_projects::thread_places(rt, origin, served.now()).await?
            } else {
                if !road.own_road() {
                    return refuse(socket, frame, &id, PLACES_TICKET_REFUSAL, Some("ticket"));
                }
                match rt.places() {
                    Some(places) => places.list(unsigned(served.now())).await?,
                    None => Vec::new(),
                }
            };
            let status = rt.status();
            let spent = wsp_core::status::StatusApi::spend(&*status, &rows, None).await.map_err(HostError::new)?;
            one("places", to_value(&spent)?)
        }
        RuntimeOp::UsageUsed { range, split, outside, .. } => {
            if !road.own_road() {
                return refuse(socket, frame, &id, PLACES_TICKET_REFUSAL, Some("ticket"));
            }
            one("used", to_value(&rt.usage().used(*range, *split, *outside).await?)?)
        }
        RuntimeOp::UsageAccounts { fresh, .. } => {
            if !road.own_road() {
                return refuse(socket, frame, &id, PLACES_TICKET_REFUSAL, Some("ticket"));
            }
            spread(&rt.usage().accounts(fresh.filter(|fresh| *fresh)).await?)?
        }
        RuntimeOp::UsageReset { account, credit_id, on, .. } => {
            if !road.own_road() {
                return refuse(socket, frame, &id, PLACES_TICKET_REFUSAL, Some("ticket"));
            }
            spread(&rt.usage().reset(account.clone(), credit_id.clone(), on.clone()).await?)?
        }
        RuntimeOp::PlacesReadings { place_id, workspace_id, range, .. } => {
            if !road.own_road() {
                return refuse(socket, frame, &id, PLACES_TICKET_REFUSAL, Some("ticket"));
            }
            let target = match (workspace_id, place_id) {
                (Some(workspace_id), _) => ReadingsTarget::Workspace(workspace_id.clone()),
                (None, Some(place_id)) => ReadingsTarget::Place(place_id.clone()),
                (None, None) => return refuse(socket, frame, &id, READINGS_TARGET_REFUSAL, Some("usage")),
            };
            spread(&rt.usage().readings(target, *range, origin).await?)?
        }
        RuntimeOp::ForwardsList { .. } => {
            let source = served.forwards_source();
            let mut shown = Vec::new();
            for forward in source.list() {
                if rt.workspaces().origin_refusal(&forward.workspace_id, origin.clone()).await?.is_none() {
                    shown.push(forward);
                }
            }
            one("forwards", to_value(&shown)?)
        }
        RuntimeOp::ForwardsStop { workspace_id, port, .. } => {
            if let Some(refusal) = rt.workspaces().origin_refusal(workspace_id, origin).await? {
                return Err(HostError::new(refusal));
            }
            let port = port.get();
            if !served.forwards_source().stop(workspace_id, port) {
                return Err(HostError::new(nothing_forwarding_line(u16::try_from(port).unwrap_or_default())));
            }
            Payload::new()
        }
        RuntimeOp::HostFolders { dir, hidden, repos, on, .. } => {
            let repos = *repos == Some(true);
            match on {
                Some(place) if place != HERE_PLACE_ID => {
                    if refused_off_own_road(socket, frame, msg) {
                        return Ok(Handled::Answered);
                    }
                    let listing = place_folders(&served, place, dir.clone(), *hidden, repos, origin).await?;
                    one("listing", to_value(&listing)?)
                }
                _ => {
                    let folders = wired(&served.opts.folders, NO_FOLDERS)?;
                    let road_here = !road.as_paired() && road.stamped.or(msg.origin) == Some(WorkspaceOrigin::Here);
                    let wide = road_here && road.by.is_none();
                    let ask = FolderListAsk { dir: dir.clone(), hidden: *hidden, repos: repos.then_some(true), wide: Some(wide) };
                    one("listing", to_value(&folders.list(ask).await?)?)
                }
            }
        }
        RuntimeOp::HostTerminalConfig { scheme, .. } => {
            let config = wired(&served.opts.terminal_config, NO_TERMINAL_CONFIG)?.read(*scheme).await?;
            one("config", to_value(&config)?)
        }
        RuntimeOp::AgentsRead { target, .. } => {
            if !road.own_road() {
                return refuse(socket, frame, &id, PLACES_TICKET_REFUSAL, None);
            }
            one("report", to_value(&rt.agents().read(target.clone(), origin).await?)?)
        }
        RuntimeOp::ServersTools { target, agent, name, refresh, .. } => {
            if !road.own_road() {
                return refuse(socket, frame, &id, PLACES_TICKET_REFUSAL, None);
            }
            let ask = ToolsAsk { agent: agent.clone(), name: name.clone(), refresh: *refresh };
            one("answer", to_value(&rt.agents().tools(target.clone(), ask, origin).await?)?)
        }
        RuntimeOp::ServersIcon { host, refresh, .. } => {
            if refused_off_own_road(socket, frame, msg) {
                return Ok(Handled::Answered);
            }
            one("icon", to_value(&rt.agents().servers_icon(host, *refresh).await?)?)
        }
        RuntimeOp::AgentsSignInLine { target, agent, name, .. } => {
            if !road.own_road() {
                return refuse(socket, frame, &id, PLACES_TICKET_REFUSAL, Some("ticket"));
            }
            if !road.host_only() {
                return refuse(socket, frame, &id, SIGN_IN_LINE_REFUSAL, None);
            }
            let ask = SignInAsk { agent: agent.clone(), server: name.clone(), terminal: None, scope: None, project: None };
            one("line", to_value(&rt.agents().sign_in_line(target.clone(), ask, origin).await?)?)
        }
        RuntimeOp::AgentsKey { agent, key, way, .. } => {
            if !road.host_only() {
                return refuse(socket, frame, &id, AGENTS_KEY_REFUSAL, None);
            }
            let way = way.as_ref().map(|way| match way {
                wsp_wire::wire::runtime::AgentsKeyWay::Token => KeyWay::Token,
                wsp_wire::wire::runtime::AgentsKeyWay::Key => KeyWay::Key,
            });
            rt.agents().key(agent, key, way).await?;
            Payload::new()
        }
        RuntimeOp::AgentsSetup { place_id, agent, on, program, config_dir, args, env, .. } => {
            if refused_off_own_road(socket, frame, msg) {
                return Ok(Handled::Answered);
            }
            if !road.host_only() && env.as_ref().is_some_and(|env| env.values().any(|value| value.is_some())) {
                return refuse(socket, frame, &id, ENV_VALUE_REFUSAL, None);
            }
            let change = AgentSetupSet {
                on: *on,
                program: program.clone(),
                config_dir: config_dir.clone(),
                args: args.clone(),
                env: env.as_ref().map(|env| env.iter().map(|(name, value)| (EnvName(name.clone()), value.clone())).collect()),
            };
            one("agent", to_value(&rt.agents().setup(place_id, agent, change, origin).await?)?)
        }
        RuntimeOp::AgentsAddTools { target, agent, .. } => {
            if refused_off_own_road(socket, frame, msg) {
                return Ok(Handled::Answered);
            }
            let file = rt.agents().add_tools(target.clone(), agent, origin).await?;
            one("file", Value::from(file))
        }
        RuntimeOp::SkillsSearch { q, limit, .. } => {
            if refused_off_own_road(socket, frame, msg) {
                return Ok(Handled::Answered);
            }
            let limit = limit.and_then(unsigned_int).and_then(|limit| u32::try_from(limit).ok());
            one("skills", to_value(&rt.agents().skills_search(q, limit).await?)?)
        }
        RuntimeOp::SkillsGet { skill, .. } => {
            if refused_off_own_road(socket, frame, msg) {
                return Ok(Handled::Answered);
            }
            one("preview", to_value(&rt.agents().skills_get(skill).await?)?)
        }
        RuntimeOp::SkillsPreview { target, name, project, .. } => {
            if refused_off_own_road(socket, frame, msg) {
                return Ok(Handled::Answered);
            }
            let ask = SkillAsk { name: name.clone(), project: *project };
            one("preview", to_value(&rt.agents().skills_preview(target.clone(), ask, origin).await?)?)
        }
        RuntimeOp::SkillsAdd { target, skill, agents, project, .. } => {
            if refused_off_own_road(socket, frame, msg) {
                return Ok(Handled::Answered);
            }
            let ask = SkillsAddAsk { skill: skill.clone(), agents: agents.clone(), project: *project };
            one("added", to_value(&rt.agents().skills_add(target.clone(), ask, origin).await?)?)
        }
        RuntimeOp::SkillsRemove { target, name, project, .. } => {
            if refused_off_own_road(socket, frame, msg) {
                return Ok(Handled::Answered);
            }
            let ask = SkillAsk { name: name.clone(), project: *project };
            one("removed", to_value(&rt.agents().skills_remove(target.clone(), ask, origin).await?)?)
        }
        RuntimeOp::SkillsToggle { target, name, project, on, .. } => {
            if refused_off_own_road(socket, frame, msg) {
                return Ok(Handled::Answered);
            }
            let ask = SkillAsk { name: name.clone(), project: *project };
            one("paths", to_value(&rt.agents().skills_toggle(target.clone(), ask, *on, origin).await?)?)
        }
        RuntimeOp::ServersAdd { target, agent, name, project, command, args, env, url, headers, .. } => {
            if refused_off_own_road(socket, frame, msg) {
                return Ok(Handled::Answered);
            }
            let ask = ServerAdd {
                agent: agent.clone(),
                name: name.clone(),
                project: *project,
                command: command.clone(),
                args: args.clone(),
                env: env.clone(),
                url: url.clone(),
                headers: headers.clone(),
            };
            let file = rt.agents().servers_add(target.clone(), ask, origin).await?;
            one("file", Value::from(file))
        }
        RuntimeOp::ServersRemove { target, agent, name, scope, .. } => {
            if refused_off_own_road(socket, frame, msg) {
                return Ok(Handled::Answered);
            }
            let ask = ServerAsk { agent: agent.clone(), name: name.clone(), scope: *scope };
            let file = rt.agents().servers_remove(target.clone(), ask, origin).await?;
            one("file", Value::from(file))
        }
        RuntimeOp::ServersToggle { target, agent, name, scope, on, .. } => {
            if refused_off_own_road(socket, frame, msg) {
                return Ok(Handled::Answered);
            }
            let ask = ServerAsk { agent: agent.clone(), name: name.clone(), scope: *scope };
            let file = rt.agents().servers_toggle(target.clone(), ask, *on, origin).await?;
            one("file", Value::from(file))
        }
        RuntimeOp::PluginsToggle { target, agent, plugin, on, .. } => {
            if refused_off_own_road(socket, frame, msg) {
                return Ok(Handled::Answered);
            }
            let ask = PluginAsk { agent: agent.clone(), plugin: plugin.clone() };
            one("plugin", to_value(&rt.agents().plugins_toggle(target.clone(), ask, *on, origin).await?)?)
        }
        RuntimeOp::EditorList { .. } => {
            if !road.own_road() {
                return refuse(socket, frame, &id, EDITOR_TICKET_REFUSAL, Some("ticket"));
            }
            one("editors", to_value(&wired(&served.opts.editor, NO_EDITOR)?.list().await?)?)
        }
        RuntimeOp::EditorOpen { workspace_id, path, line, editor, .. } => {
            if !road.own_road() {
                return refuse(socket, frame, &id, EDITOR_TICKET_REFUSAL, Some("ticket"));
            }
            let workspace = rt.workspaces().get(workspace_id, origin.clone(), None).await?;
            let picked = match editor {
                Some(editor) => Some(*editor),
                None => rt.preferences().get().await?.editor,
            };
            let line = line.and_then(unsigned_int);
            if is_local_workspace(workspace.kind.as_ref()) {
                let inside = [workspace.worktree.as_ref().map(|worktree| worktree.path.clone()), Some(workspace.project.path.clone())]
                    .into_iter()
                    .flatten()
                    .collect();
                let ask = EditorOpen { path: path.clone(), line, inside, editor: picked, remote: None };
                one("editor", to_value(&wired(&served.opts.editor, NO_EDITOR)?.open(ask).await?)?)
            } else {
                let Some(ssh) = served.opts.ssh.clone() else {
                    return Err(HostError::new(editor_opens_here_line(&workspace.name)));
                };
                let editors = wired(&served.opts.editor, NO_EDITOR)?;
                if let Some(refused) = editors.remote_refusal(picked, &workspace.name).await? {
                    return Err(HostError::new(refused));
                }
                if !ssh.include().await? {
                    return refuse(socket, frame, &id, &ssh_include_line(&workspace.name), Some("sshInclude"));
                }
                let (woken_id, woken_name) = awake_for_ssh(&served, &workspace, origin).await?;
                ssh.port(&woken_id, &woken_name).await?;
                let remote = EditorRemote {
                    alias: ssh_alias(&workspace.name),
                    folder: workspace.folder.clone().unwrap_or_else(|| workspace.project.path.clone()),
                    name: workspace.name.clone(),
                };
                let ask = EditorOpen { path: path.clone(), line, inside: Vec::new(), editor: picked, remote: Some(remote) };
                one("editor", to_value(&editors.open(ask).await?)?)
            }
        }
        RuntimeOp::SshPort { workspace_id, .. } => {
            if !road.own_road() {
                return refuse(socket, frame, &id, SSH_TICKET_REFUSAL, Some("ticket"));
            }
            let door = wired(&served.opts.ssh, NO_SSH)?;
            let held = rt.workspaces().get(workspace_id, origin.clone(), None).await?;
            if is_local_workspace(held.kind.as_ref()) {
                return Err(HostError::new(ssh_copy_here_line(&held.name)));
            }
            let (woken_id, woken_name) = awake_for_ssh(&served, &held, origin).await?;
            one("port", json!(door.port(&woken_id, &woken_name).await?))
        }
        RuntimeOp::SshInclude { on, .. } => {
            if !road.own_road() {
                return refuse(socket, frame, &id, SSH_TICKET_REFUSAL, Some("ticket"));
            }
            let door = wired(&served.opts.ssh, NO_SSH)?;
            let included = match on {
                None => door.include().await?,
                Some(on) => door.set_include(*on).await?,
            };
            one("sshInclude", json!(included))
        }
        RuntimeOp::InitGet { on, .. } => {
            let setup = wired(&served.opts.init, NO_INIT)?.get(on.clone()).await?;
            one("setup", to_value(&setup)?)
        }
        RuntimeOp::InitKeys { provider, key, .. } => {
            let setup = wired(&served.opts.init, NO_INIT)?.keys(provider.clone(), key.clone()).await?;
            one("setup", to_value(&setup)?)
        }
        RuntimeOp::PreferencesGet { .. } => one("preferences", to_value(&rt.preferences().get().await?)?),
        RuntimeOp::PreferencesSet { patch, .. } => {
            let set = rt.preferences().set(patch.clone()).await?;
            let mut out = one("preferences", to_value(&set.preferences)?);
            if let Some(notice) = set.notice {
                out.insert("notice".into(), Value::from(notice));
            }
            out
        }
        RuntimeOp::ReleaseGet { .. } => one("release", to_value(&wired(&served.opts.release, NO_RELEASE)?.get())?),
        RuntimeOp::ReleaseCheck { force, .. } => {
            let release = wired(&served.opts.release, NO_RELEASE)?.check(*force == Some(true)).await?;
            one("release", to_value(&release)?)
        }
        RuntimeOp::HostRestart { .. } => {
            if !road.own_road() {
                return refuse(socket, frame, &id, HOST_RESTART_TICKET_REFUSAL, Some("ticket"));
            }
            let door = wired(&served.opts.restart, HOST_NO_RESTART_LINE)?;
            if let Some(refusal) = door.refusal() {
                return Err(HostError::new(refusal));
            }
            socket.answer(Some(frame), ok_reply(&id, Payload::new()), None);
            start_restart(&served, door);
            return Ok(Handled::Answered);
        }
        RuntimeOp::ProjectSeedPlan { source, on, .. } => one("plan", to_value(&rt.projects().seed_plan(source, on.clone()).await?)?),
        RuntimeOp::ProjectPlan { source, .. } => {
            let bundler = bundler_from(&served, source)?;
            one("plan", to_value(&bundler.plan().await?)?)
        }
        RuntimeOp::ProjectImport { workspace_id, source, dest, replace, carry, rewrite, agents, .. } => {
            let bundler = bundler_from(&served, source)?;
            let ask = ProjectImportOptions {
                workspace_id: workspace_id.clone(),
                source: source.clone(),
                dest: dest.clone(),
                replace: *replace,
                carry: carry.clone(),
                rewrite: rewrite.clone(),
                agents: agents.clone(),
                bundler,
            };
            one("imported", to_value(&rt.projects().import(ask, origin).await?)?)
        }
        RuntimeOp::ProjectExport { workspace_id, source, dest, replace, agents, .. } => {
            let lander: Rc<dyn ProjectLander> = wired(&served.opts.landing, NO_LANDER)?;
            let ask = ProjectExportOptions {
                workspace_id: workspace_id.clone(),
                source: source.clone(),
                dest: dest.clone(),
                replace: *replace,
                agents: agents.clone(),
                lander,
            };
            one("exported", to_value(&rt.projects().export(ask, origin).await?)?)
        }
        _ => return Ok(Handled::NotMine),
    };
    socket.answer(Some(frame), ok_reply(&id, fields), None);
    Ok(Handled::Answered)
}

/// One level of folders on a joined computer, read by its own daemon over its link (serve.ts:419-440, spec O15).
pub async fn place_folders(
    served: &Served,
    place_id: &str,
    dir: Option<String>,
    hidden: Option<bool>,
    repos: bool,
    origin: Option<Caller>,
) -> Result<HostFolderListing, HostError> {
    let door = served.rt.places().ok_or_else(|| HostError::new(NO_PLACE_DOOR))?;
    let rows = door.list(unsigned(served.now())).await?;
    let Some(row) = rows.iter().find(|place| place.id == place_id).cloned() else {
        let held: Vec<String> = rows.iter().map(table_name_of).collect();
        return Err(HostError::with_kind(no_such_place_refusal(place_id, &held), Kind::Usage));
    };
    if !is_joined_computer(&row.id, kind_word(&row.kind)) {
        return Err(HostError::with_kind(provider_folders_refusal(&table_name_of(&row)), Kind::Usage));
    }
    let version = door.report_of(&row.id).await?.map(|report| report.daemon_version.get());
    if let Some(word) = version.filter(|version| *version < FS_FOLDERS_DAEMON_VERSION).and_then(|v| place_daemon_behind(Some(v))) {
        return Err(HostError::new(place_behind_line(&row.name, &word)));
    }
    let Some(link) = door.channel(&row.id, Rc::new(|_| {})) else {
        return Err(HostError::new(absent_computer(&row.name, None).sentence));
    };
    let answer = async {
        let projects: Vec<String> = served
            .rt
            .projects()
            .list(origin)
            .await?
            .into_iter()
            .filter(|project| project.computer == row.id)
            .map(|project| project.checkout.unwrap_or(project.path))
            .collect();
        let mut frame = Map::new();
        frame.insert("op".into(), Value::from("fs.folders"));
        if let Some(dir) = dir {
            frame.insert("dir".into(), Value::from(dir));
        }
        if let Some(hidden) = hidden {
            frame.insert("hidden".into(), Value::from(hidden));
        }
        if repos {
            frame.insert("repos".into(), Value::from(true));
        }
        frame.insert("projects".into(), json!(projects));
        let reply = link.send(frame).await.map_err(|e| HostError::new(e.to_string()))?;
        if reply.get("ok") != Some(&Value::Bool(true)) {
            let error = reply.get("error").and_then(Value::as_str).unwrap_or_default();
            return Err(HostError::new(error));
        }
        parse::<HostFolderListing>(&Value::Object(reply)).map_err(|e| HostError::new(e.issue_text()))
    }
    .await;
    link.close();
    answer
}

/// `forwards.stop`'s refusal for a forward the source does not hold. serve.ts:1652.
pub fn nothing_forwarding_line(port: u16) -> String {
    format!("nothing is forwarding localhost:{port} there")
}

/// `bundlerFrom(opts)(source)`: the bundler the host wired, or the refusal naming there is none (serve.ts:333-338).
fn bundler_from(served: &Served, source: &str) -> Result<Rc<dyn ProjectBundler>, HostError> {
    let make = served.opts.projects.as_ref().ok_or_else(|| HostError::new(NO_BUNDLER))?;
    Ok(make(source))
}

/// `doorFrom(opts, key, missing)()`: the door the host wired, or the refusal naming what is missing (serve.ts:340-347).
fn wired<T: ?Sized>(door: &Option<Rc<T>>, missing: &str) -> Result<Rc<T>, HostError> {
    door.clone().ok_or_else(|| HostError::new(missing))
}

/// `awakeForSsh(held)`: the workspace as its ssh door takes it, woken first where it is not running (serve.ts:857-862).
async fn awake_for_ssh(served: &Served, held: &WorkspaceView, origin: Option<Caller>) -> Result<(String, String), HostError> {
    if matches!(held.phase, WorkspacePhase::Running) {
        return Ok((held.id.clone(), held.name.clone()));
    }
    let woken = served.rt.workspaces().wake(&held.id, origin).await?;
    Ok((woken.id, woken.name))
}

/// Starts the host's restart once; asks that overlap join it, and a restart that fails forgets itself (serve.ts:1832-1842).
fn start_restart(served: &Rc<Served>, door: Rc<dyn super::RestartDoor>) {
    if served.restarting.borrow().is_some() {
        return;
    }
    let owner = served.clone();
    let restart = async move {
        if let Err(e) = door.restart().await {
            *owner.restarting.borrow_mut() = None;
            if let Some(log) = &owner.opts.log {
                log(&format!("host.restart failed: {}", e.message));
            }
        }
    }
    .boxed_local()
    .shared();
    *served.restarting.borrow_mut() = Some(restart.clone());
    tokio::task::spawn_local(restart);
}

/// Sends a refusal the op answers with, and says the frame was answered.
fn refuse(socket: &Socket, frame: &Value, id: &Value, error: &str, kind: Option<&str>) -> Result<Handled, HostError> {
    socket.answer(Some(frame), refusal_reply(id, error, kind), None);
    Ok(Handled::Answered)
}

/// `{key: value}`: a reply's one field.
fn one(key: &str, value: Value) -> Payload {
    let mut out = Payload::new();
    out.insert(key.into(), value);
    out
}

fn to_value<T: Serialize>(answer: &T) -> Result<Value, HostError> {
    serde_json::to_value(answer).map_err(|e| HostError::new(e.to_string()))
}

/// The fields of an answer spread beside id and ok, in the answer's own order.
fn spread<T: Serialize>(answer: &T) -> Result<Payload, HostError> {
    match to_value(answer)? {
        Value::Object(fields) => Ok(fields),
        _ => Err(HostError::new("the answer is not an object")),
    }
}

/// `spread` for a start's answer, which the frame it answers cannot fail to send.
fn spread_or_empty<T: Serialize>(answer: &T) -> Payload {
    spread(answer).unwrap_or_default()
}

fn unsigned(ms: i64) -> u64 {
    u64::try_from(ms).unwrap_or(0)
}

fn unsigned_int(value: wsp_wire::types::js_number::JsInt) -> Option<u64> {
    u64::try_from(value.get()).ok()
}

fn kind_word(kind: &wsp_wire::views::place_kind::PlaceKind) -> &'static str {
    match kind {
        wsp_wire::views::place_kind::PlaceKind::Computer => "computer",
        wsp_wire::views::place_kind::PlaceKind::Provider => "provider",
    }
}

/// `tableName(place)`: what a table calls a computer (place-name.ts:35).
fn table_name_of(place: &wsp_wire::views::place_more::PlaceView) -> String {
    table_name(&PlaceRef { id: place.id.clone(), name: place.name.clone(), kind: Some(kind_word(&place.kind).to_string()) })
}

/// `startAt`'s workspaces as a sized value: `start_at` takes a `&dyn`, and a `&dyn WorkspacesApi` cannot be coerced to one.
struct WorkspacesAt<'a>(&'a dyn WorkspacesApi);

#[async_trait(?Send)]
impl StartAtWorkspaces for WorkspacesAt<'_> {
    async fn folder_for(
        &self,
        ask: wsp_core::types::api::FolderForAsk,
        origin: Option<Caller>,
    ) -> Result<wsp_core::types::api::FolderFor, HostError> {
        WorkspacesApi::folder_for(self.0, ask, origin).await
    }

    async fn take_setup_line(&self, workspace_id: &str, origin: Option<Caller>) -> Result<Option<String>, HostError> {
        WorkspacesApi::take_setup_line(self.0, workspace_id, origin).await
    }
}
