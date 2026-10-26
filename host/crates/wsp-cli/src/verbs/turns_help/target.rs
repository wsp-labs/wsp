// SPDX-License-Identifier: AGPL-3.0-only
//! `turns-help.ts:238-420` at 65f6b89c7: where a run goes and the threads a line names. Spec: `spec/turns-target.md`.
//! The drivers in `tests/cli_c/drivers_turns_target.rs` and the cases of
//! `tests/fixtures/cli_c/turns_target.json` say what each one does. `thread_at` is the signature other units call (fixed by
//! the lead); the cloud flag the TS reads from `CLOUD_ON` is a parameter on the helpers that read it, so a test can run
//! both settings in one process (`thread_at` itself reads `crate::cloud::cloud_on()` and is `thread_at_in` with it).

use std::future::Future;
use std::sync::Arc;

use async_trait::async_trait;
use indexmap::{IndexMap, IndexSet};
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};
use tokio::sync::mpsc::{unbounded_channel, UnboundedReceiver};

use wsp_wire::types::check::parse;
use wsp_wire::types::env::HOST_TOKEN_ENV;
use wsp_wire::types::projects::BRANCH_HERE_ONLY_LINE;
use wsp_wire::types::sub_thread::SubStarted;
use wsp_wire::views::activity::{ActivityEnded, ProcessesAsk};
use wsp_wire::views::session::ThreadView;
use wsp_wire::views::session_events::turn_token_of;
use wsp_wire::views::workspace::ProjectView;
use wsp_wire::views::workspace_kind::WorkspaceKind;
use wsp_wire::views::workspace_more::WorkspaceOut;
use wsp_words::exit::{refusal, usage_refusal, HostError, Kind};
use wsp_words::projects::{copies_folder, home_shortened, kind_for_computer, runs_in_folder, thread_on_machine_line, thread_opened_line};
use wsp_words::start::{name_of_task, taken_name_after};
use wsp_words::workspace_state::workspace_kind;

use wsp_words::words::computer::{GUEST_NAMES_FIX, GUEST_NAMES_WORKSPACE_LINE};
use wsp_words::words::thread::{
    child_beside_lead_line, no_project_line, not_a_thread_line, WordIs, CHILD_BESIDE_LEAD_FIX, NAME_A_THREAD_FIX, READ_PROJECTS_FIX,
};

use super::picks::reply_field;
use crate::cloud::cloud_on;
use crate::repo_root::main_worktree_of;
use crate::verbs::client::{other_version, pick, thread_id_of, under, CliIo, Frame, HostClient, Out, PushedFrames};
use crate::verbs::workspaces_help::{
    create_for, project_of, projects_here, projects_of, thread_of, threads, workspace_of, workspaces, CreateAsked,
    ProjectPick as HostProjectPick,
};

/// A project as the host answered it, kept whole: the TS hands on whatever object `projects.resolve`, `projects.list` or
/// a workspace's project gave, so `id` and `name` come first, then `computer` where the object carries one, then the rest
/// in the order the host wrote it. A folder target's `project` read by a ref is the three fields `id`, `name`, `path`
/// (no `computer`); read off the caller's folder or a fork's it is the whole `ProjectView`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ProjectPick {
    pub id: String,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub computer: Option<String>,
    #[serde(flatten)]
    pub rest: Map<String, Value>,
}

/// The line a target says for a thread it opened (`opened` of `FolderTarget`, turns-help.ts:233): the thread id and the
/// folder it opened in, if any.
pub type Opened = Arc<dyn Fn(&str, Option<&str>) -> String + Send + Sync>;

/// The `here` of a `FolderTarget` (turns-help.ts:232): a thread on this computer as a start names it: the project (else the
/// thread asking), and the branch or the folder inside it. Written project, branch, cwd; absent is left out.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct FolderHere {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project: Option<ProjectPick>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub branch: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cwd: Option<String>,
}

/// `FolderTarget` (turns-help.ts:231-234): `here`, and `opened(threadId, folder?)`, which says `thread <id>` with no folder
/// and `thread_opened_line(id, name, folder)` with one (the folder shortened against HOME where the project is on this
/// computer or the target is the thread asking's, as is; never for a folder on another computer).
#[derive(Clone)]
pub struct FolderTarget {
    pub here: FolderHere,
    pub opened: Opened,
}

/// `ForkTarget` (turns-help.ts:237-239): a run on a project that lives on a box or a cloud account: a machine forked for
/// it once the start is checked.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ForkTarget {
    pub fork: ProjectPick,
}

/// `RunTarget` (turns-help.ts:241): the three answers `run_target` gives. A workspace answer carries no `opened`.
#[derive(Clone)]
pub enum RunTarget {
    Folder(FolderTarget),
    Fork(ForkTarget),
    Workspace { workspace: WorkspaceOut, opened: Option<Opened> },
}

/// `where` of `runTarget` (turns-help.ts:253): the `--branch` and `--cwd` of the line; an empty string is named.
#[derive(Clone, Debug, Default, Deserialize)]
pub struct RunWhere {
    pub branch: Option<String>,
    pub cwd: Option<String>,
}

/// `runTarget(client, ref, callerCwd, env, elsewhere, where)` (turns-help.ts:247-292): where a run goes. `cloud_on` is the
/// TS's `CLOUD_ON`. `HOME` is read for `opened`'s shortening.
///
/// With a `ref` (read before any token):
/// - the project is `projects.resolve {ref}`, else (that refused) the first of `projects.list` (the thread's own
///   workspaces' projects from `workspaces.list` where the list is refused) whose id or name is `ref`;
/// - a project on this computer (`kind_for_computer` copies a folder), or one whose `workspaces.landing {project}`
///   kind runs in a folder (a refused or kindless landing is no), is a folder target: `projects.resolve {ref: project.id}`
///   again for its `path` ("" when that read is refused), `branch` then `cwd` as asked;
/// - any other project: a `--branch` is refused (`BRANCH_HERE_ONLY_LINE`, fix "Name a project on this computer."); without
///   the cloud, or with it and no machine of that name in `workspaces.list`, the answer is `Fork(project)`; with a machine of
///   the name the word goes on as one;
/// - no project, no cloud: `projects.resolve` is read once more and its refusal, unless of kind `not-found`, is the answer;
///   then `no_project_line(ref)` + `READ_PROJECTS_FIX`, kind `not-found`;
/// - no project, with the cloud: `workspace_of(ref)` (`workspaces.resolve`), then a `--branch` is refused, else `Workspace`.
///
/// With no `ref`: a `WSP_TURN` or `WSP_HOST_TOKEN` in `env` (non-empty) is a folder target beside the thread asking,
/// `here` carrying only `branch` and `cwd`, and nothing is asked; else `elsewhere` is `guest_names_workspace_line` +
/// `GUEST_NAMES_FIX` (nothing asked); else the project is `project_of_folder(projects.list or [], callerCwd)`: none is
/// `no_thread_target_line("<project>")` + `READ_PROJECTS_FIX`; one gives `here` the whole project, `branch`, `cwd`, and the
/// caller's own folder as `cwd` where the caller is outside the project's folder (a worktree) and neither was named.
pub async fn run_target(
    client: &dyn HostClient,
    r#ref: Option<&str>,
    caller_cwd: Option<&str>,
    env: &IndexMap<String, String>,
    elsewhere: Option<bool>,
    r#where: &RunWhere,
    cloud_on: bool,
) -> Result<RunTarget, HostError> {
    let branch = r#where.branch.clone();
    let cwd = r#where.cwd.clone();
    if let Some(r#ref) = r#ref {
        let project = match project_of(client, r#ref).await {
            Ok(project) => Some(pick_of(&project)),
            Err(_) => projects_here(client)
                .await
                .unwrap_or_default()
                .into_iter()
                .find(|p| p.id == r#ref || p.name == r#ref)
                .map(|p| ProjectPick { id: p.id, name: p.name, computer: Some(p.computer), rest: Map::new() }),
        };
        if let Some(project) = &project {
            let copies = copies_folder(&kind_for_computer(project.computer.as_deref().unwrap_or(""), None));
            if copies || threads_in_folder(client, &project.id).await {
                let path = project_of(client, &project.id).await.map(|full| full.path).unwrap_or_default();
                let here = FolderHere { project: Some(folder_project(project.id.clone(), project.name.clone(), path)), branch, cwd };
                return Ok(RunTarget::Folder(FolderTarget { here, opened: opened_of(Some(project.name.clone()), copies) }));
            }
        }
        let branch_refused = || usage_refusal(BRANCH_HERE_ONLY_LINE, "Name a project on this computer.");
        if let Some(project) = project {
            if r#where.branch.is_some() {
                return Err(branch_refused());
            }
            if !cloud_on || !workspaces(client).await?.iter().any(|w| w.name == r#ref) {
                return Ok(RunTarget::Fork(ForkTarget { fork: project }));
            }
        }
        if !cloud_on {
            if let Err(e) = project_of(client, r#ref).await {
                if e.kind != Some(Kind::NotFound) {
                    return Err(e);
                }
            }
            return Err(refusal(&no_project_line(r#ref), READ_PROJECTS_FIX, Some(Kind::NotFound)));
        }
        let workspace = workspace_of(client, r#ref).await?;
        if r#where.branch.is_some() {
            return Err(branch_refused());
        }
        return Ok(RunTarget::Workspace { workspace, opened: None });
    }
    let token = turn_token_of(&to_hash_map(env));
    if token.is_some() || env.get(HOST_TOKEN_ENV).is_some_and(|v| !v.is_empty()) {
        return Ok(RunTarget::Folder(FolderTarget { here: FolderHere { project: None, branch, cwd }, opened: opened_of(None, true) }));
    }
    if elsewhere == Some(true) {
        return Err(usage_refusal(GUEST_NAMES_WORKSPACE_LINE, GUEST_NAMES_FIX));
    }
    let project = match caller_cwd {
        None => None,
        Some(folder) => project_of_folder(&projects_of(client).await.unwrap_or_default(), folder),
    };
    let Some(project) = project else {
        return Err(usage_refusal(&wsp_words::projects::no_thread_target_line("<project>"), READ_PROJECTS_FIX));
    };
    let inside = caller_cwd.is_some_and(|folder| !under(folder, &project.path));
    let cwd = if inside && r#where.cwd.is_none() && r#where.branch.is_none() { caller_cwd.map(str::to_owned) } else { cwd };
    let name = project.name.clone();
    let here = FolderHere { project: Some(pick_of(&project)), branch, cwd };
    Ok(RunTarget::Folder(FolderTarget { here, opened: opened_of(Some(name), true) }))
}

/// Whether a thread of this project runs in its folder on the computer holding it, as a project on a computer the
/// person joined does: the host answers off the project's landing, since only it holds which computers are joined.
pub(crate) async fn threads_in_folder(client: &dyn HostClient, project: &str) -> bool {
    let Ok(landing) = client.request("workspaces.landing", pick(vec![("project", Some(json!(project)))])).await else {
        return false;
    };
    landing
        .get("kind")
        .and_then(|kind| serde_json::from_value::<WorkspaceKind>(kind.clone()).ok())
        .is_some_and(|kind| runs_in_folder(&kind))
}

/// A machine forked for a run on a project elsewhere, the way the app's New thread makes one: named off the task, with
/// a number after it where a machine already has that name, as wsp start numbers one. The stages it streams go to
/// `say`. Called last, once everything else on the line has been read, so a refusal costs no machine.
pub async fn fork_for(
    client: &dyn HostClient,
    project: &ProjectPick,
    task: &str,
    say: &(dyn Fn(&str) + Send + Sync),
) -> Result<WorkspaceOut, HostError> {
    let taken = taken_names(client).await?;
    let name = taken_name_after(&name_of_task(task), &taken);
    let (tx, mut lines) = unbounded_channel::<String>();
    let out = Out::formatter(Arc::new(QueuedIo(tx)), false);
    let host_pick =
        HostProjectPick { id: project.id.clone(), name: project.name.clone(), computer: project.computer.clone().unwrap_or_default() };
    let created = saying(&mut lines, say, create_for(client, &out, &host_pick, &name, &CreateAsked::default())).await?;
    Ok(created.workspace)
}

/// The terminal a create's stages and lines are queued on, for `saying` to say them as they land.
struct QueuedIo(tokio::sync::mpsc::UnboundedSender<String>);

#[async_trait]
impl CliIo for QueuedIo {
    fn log(&self, line: &str) {
        let _ = self.0.send(line.to_owned());
    }

    fn error(&self, line: &str) {
        let _ = line;
    }

    fn stream(&self, text: &str) {
        let _ = self.0.send(text.trim_end().to_owned());
    }

    async fn ask(&self, question: &str) -> Result<String, HostError> {
        Err(HostError::new(format!("unexpected prompt: {question}")))
    }

    async fn ask_secret(&self, question: &str, _variable: Option<&str>) -> Result<String, HostError> {
        Err(HostError::new(format!("unexpected prompt: {question}")))
    }
}

/// Runs `work`, saying each line `lines` holds to `say` as it lands, and the lines still held once `work` answers.
async fn saying<T>(
    lines: &mut UnboundedReceiver<String>,
    say: &(dyn Fn(&str) + Send + Sync),
    work: impl Future<Output = Result<T, HostError>>,
) -> Result<T, HostError> {
    tokio::pin!(work);
    loop {
        tokio::select! {
            biased;
            answer = &mut work => {
                while let Ok(line) = lines.try_recv() {
                    say(&line);
                }
                return answer;
            }
            Some(line) = lines.recv() => say(&line),
        }
    }
}

/// A sub-thread's machine on a cloud, named off the task as a fork's is unless the title names it, with a number
/// after it where a machine already has that name. The host reads the asking thread's project off its token and every
/// check before the cloud is asked for a start. Called last, once everything else on the line has been read, so a
/// refusal costs no machine. The machine is made once the thread's start comes, so its stages go to `say` after the
/// answer, until `stop`.
pub async fn sub_for(
    client: &dyn HostClient,
    ask: &SubAsk,
    task: &str,
    title: Option<&str>,
    say: &(dyn Fn(&str) + Send + Sync),
) -> Result<SubMade, HostError> {
    let taken = taken_names(client).await?;
    let name = taken_name_after(&title.map_or_else(|| name_of_task(task), str::to_owned), &taken);
    let pushed = Arc::new(PushedFrames::new(client));
    client.events().await?;
    let (tx, mut lines) = unbounded_channel::<String>();
    let watched = name.clone();
    pushed.follow(
        Arc::new(move |f: &Frame| f.get("type") == Some(&json!("workspace.creating")) && f.get("name") == Some(&json!(watched))),
        Arc::new(move |f: &Frame| {
            let _ = tx.send(f.get("message").and_then(Value::as_str).unwrap_or_default().to_owned());
        }),
    );
    let mut sub_params = Map::new();
    sub_params.insert("on".into(), json!(ask.on));
    for (key, value) in [("size", &ask.size), ("from", &ask.from), ("branch", &ask.branch), ("harness", &ask.harness)] {
        if let Some(value) = value {
            sub_params.insert(key.into(), json!(value));
        }
    }
    let params = pick(vec![
        ("project", Some(json!(ask.project.as_deref().unwrap_or("")))),
        ("name", Some(json!(name))),
        ("sub", Some(Value::Object(sub_params))),
    ]);
    let made = async {
        let frame = saying(&mut lines, say, client.request("workspaces.create", params)).await?;
        if let Some(notice) = frame.get("notice").and_then(Value::as_str) {
            for line in notice.split('\n') {
                say(line);
            }
        }
        let workspace = reply_field::<WorkspaceOut>(&frame, "workspaces.create", "workspace")?;
        let sub = parse::<SubStarted>(frame.get("sub").unwrap_or(&Value::Null))
            .map_err(|_| HostError::new(other_version("workspaces.create")))?;
        Ok::<_, HostError>((workspace, sub))
    };
    match made.await {
        Ok((workspace, sub)) => Ok(SubMade { workspace, sub, stop: Box::new(move || pushed.stop()) }),
        Err(e) => {
            pushed.stop();
            Err(e)
        }
    }
}

/// The project on this computer a folder is in: the one whose folder holds it, the deepest where two do, else the one
/// whose repo the folder is a worktree of.
pub fn project_of_folder(projects: &[ProjectView], folder: &str) -> Option<ProjectView> {
    let here: Vec<&ProjectView> = projects.iter().filter(|p| copies_folder(&kind_for_computer(&p.computer, None))).collect();
    let mut holding: Option<&ProjectView> = None;
    for project in here.iter().copied().filter(|p| under(folder, &p.path)) {
        if holding.is_none_or(|best| path_length(&project.path) > path_length(&best.path)) {
            holding = Some(project);
        }
    }
    if let Some(holding) = holding {
        return Some(holding.clone());
    }
    let main = main_worktree_of(folder)?;
    here.into_iter().find(|p| p.git.as_ref().is_some_and(|git| git.top == main)).cloned()
}

/// What `threadHere` is handed (turns-help.ts:351): a word, or the thread row already read.
pub enum ThreadRef<'a> {
    Word(&'a str),
    Row(&'a ThreadView),
}

/// What `threadHere` answers (turns-help.ts:351): the thread's full id, its workspace, and the worktree wsp made for it.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ThreadHere {
    pub id: String,
    pub workspace_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub worktree: Option<String>,
}

/// `threadHere(client, ref)` (turns-help.ts:351-357): a thread on this computer a word names (`thread_of`; none for a
/// word that names no thread or several, nothing sent for no ref, no listing for a row already read), when its workspace
/// (`workspaces.list`) runs in a folder (`runs_in_folder(workspace_kind)`; a workspace of no kind is a machine) with
/// `worktree` the made, not-gone worktree's path.
pub async fn thread_here(client: &dyn HostClient, r#ref: Option<ThreadRef<'_>>) -> Result<Option<ThreadHere>, HostError> {
    let thread = match r#ref {
        None => None,
        Some(ThreadRef::Word(word)) => thread_of(client, word).await.ok(),
        Some(ThreadRef::Row(row)) => Some(row.clone()),
    };
    let Some(thread) = thread else { return Ok(None) };
    let Some(at) = workspaces(client).await?.into_iter().find(|w| w.id == thread.workspace_id) else { return Ok(None) };
    if !runs_in_folder(&workspace_kind(at.kind.as_ref())) {
        return Ok(None);
    }
    let worktree = at.worktree.as_ref().filter(|w| w.made && w.gone.is_none()).map(|w| w.path.clone());
    Ok(Some(ThreadHere { id: thread_id_of(&thread), workspace_id: at.id, worktree }))
}

/// What `subDeleting` answers (turns-help.ts:361).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SubDeleting {
    pub id: String,
    pub workspace_id: String,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub question: Option<String>,
}

/// `subDeleting(client, thread)` (turns-help.ts:361-366): nothing for no thread or one with no `sub` (nothing sent); else
/// `sessions.delete {threadId, check: true}` and its `question`, with the thread's full id and workspace.
pub async fn sub_deleting(client: &dyn HostClient, thread: Option<&ThreadView>) -> Result<Option<SubDeleting>, HostError> {
    let Some(thread) = thread.filter(|t| t.sub.is_some()) else { return Ok(None) };
    let id = thread_id_of(thread);
    let frame = client.request("sessions.delete", pick(vec![("threadId", Some(json!(id))), ("check", Some(json!(true)))])).await?;
    let question = frame.get("question").and_then(Value::as_str).map(str::to_owned);
    Ok(Some(SubDeleting { id, workspace_id: thread.workspace_id.clone(), question }))
}

/// Refuses a thread on a box's machine, which goes with its machine, in the runtime's own words; nothing for any
/// other ref, so the caller's own refusal stands.
pub async fn refuse_machine_thread(client: &dyn HostClient, r#ref: &str) -> Result<(), HostError> {
    let Ok(thread) = thread_of(client, r#ref).await else { return Ok(()) };
    let Some(at) = workspaces(client).await?.into_iter().find(|w| w.id == thread.workspace_id) else { return Ok(()) };
    if !runs_in_folder(&workspace_kind(at.kind.as_ref())) {
        return Err(HostError::with_kind(thread_on_machine_line(&at.name), Kind::Usage));
    }
    Ok(())
}

/// A thread a line names and the record of the folder it works in: the thread by its id or a prefix naming one, then
/// its record through the host, which refuses one the caller may not drive. A word that names no thread is read once
/// more, so a project's name, or with a cloud a machine's, is refused saying the line takes a thread, in the same
/// words on the command line and the tool.
pub async fn thread_at(
    client: &dyn HostClient,
    r#ref: &str,
    line: &str,
    or_computer: bool,
) -> Result<(ThreadView, WorkspaceOut), HostError> {
    thread_at_in(client, r#ref, line, or_computer, cloud_on()).await
}

/// `thread_at` with the cloud setting given (what the TS reads off `CLOUD_ON`).
pub async fn thread_at_in(
    client: &dyn HostClient,
    r#ref: &str,
    line: &str,
    or_computer: bool,
    cloud_on: bool,
) -> Result<(ThreadView, WorkspaceOut), HostError> {
    let thread = match thread_of(client, r#ref).await {
        Ok(thread) => thread,
        Err(e) => {
            if e.kind == Some(Kind::NotFound) {
                refuse_not_a_thread(client, r#ref, line, or_computer, cloud_on).await?;
            }
            return Err(e);
        }
    };
    let params = pick(vec![("workspaceId", Some(json!(thread.workspace_id))), ("threadId", Some(json!(thread_id_of(&thread))))]);
    let frame = client.request("workspaces.get", params).await?;
    let workspace = parse::<WorkspaceOut>(frame.get("workspace").unwrap_or(&Value::Null))
        .map_err(|_| HostError::new(other_version("workspaces.get")))?;
    Ok((thread, workspace))
}

/// `childOf(client, lead, ref, line)` (turns-help.ts:396-400): `thread_at_in(ref, line, false)`, and a child in the lead's
/// own workspace is refused `child_beside_lead_line(child id, lead id)` + `CHILD_BESIDE_LEAD_FIX` (usage).
pub async fn child_of(
    client: &dyn HostClient,
    lead: &(ThreadView, WorkspaceOut),
    r#ref: &str,
    line: &str,
    cloud_on: bool,
) -> Result<(ThreadView, WorkspaceOut), HostError> {
    let kid = thread_at_in(client, r#ref, line, false, cloud_on).await?;
    if kid.1.id == lead.1.id {
        return Err(usage_refusal(&child_beside_lead_line(&thread_id_of(&kid.0), &thread_id_of(&lead.0)), CHILD_BESIDE_LEAD_FIX));
    }
    Ok(kid)
}

/// Refuses a word that names a project, or with a cloud a machine, in the words of a line that takes a thread;
/// nothing for any other word, so the line's own refusal stands.
#[allow(dead_code)]
pub(crate) async fn refuse_not_a_thread(
    client: &dyn HostClient,
    r#ref: &str,
    line: &str,
    or_computer: bool,
    cloud_on: bool,
) -> Result<(), HostError> {
    if projects_here(client).await.unwrap_or_default().iter().any(|p| p.id == r#ref || p.name == r#ref) {
        return Err(usage_refusal(&not_a_thread_line(r#ref, WordIs::Project, line, or_computer), NAME_A_THREAD_FIX));
    }
    if !cloud_on {
        return Ok(());
    }
    if workspaces(client)
        .await
        .unwrap_or_default()
        .iter()
        .any(|w| !runs_in_folder(&workspace_kind(w.kind.as_ref())) && (w.id == r#ref || w.name == r#ref))
    {
        return Err(usage_refusal(&not_a_thread_line(r#ref, WordIs::Machine, line, or_computer), NAME_A_THREAD_FIX));
    }
    Ok(())
}

/// The other threads in a thread's folder, by id, which a commit, a discard or an update acted for too.
pub async fn sharing_with(client: &dyn HostClient, at: &(ThreadView, WorkspaceOut)) -> Result<Vec<String>, HostError> {
    let mine = thread_id_of(&at.0);
    Ok(threads(client, Some(&at.1.id))
        .await?
        .iter()
        .filter(|t| t.workspace_id == at.1.id)
        .map(thread_id_of)
        .filter(|id| *id != mine)
        .collect())
}

/// `{ processes?, check? }` of `threadDeleted` (turns-help.ts:417), sent in this order.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ThreadDeletedAsk {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub processes: Option<ProcessesAsk>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub check: Option<bool>,
}

/// The `machine` of `ThreadDeleted`: the one word the host says when the thread's machine went too.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum MachineDeleted {
    #[serde(rename = "deleted")]
    Deleted,
}

/// What `threadDeleted` answers (turns-help.ts:417): written workspaceId, worktree, threads, ended, machine, on.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ThreadDeleted {
    pub workspace_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub worktree: Option<String>,
    pub threads: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ended: Option<Vec<ActivityEnded>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub machine: Option<MachineDeleted>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub on: Option<String>,
}

/// `threadDeleted(client, threadId, o)` (turns-help.ts:417-420): `sessions.delete {threadId, processes?, check?}` and what
/// the host took, as the fields above: an `ended` the host sent (empty included) is read as `ActivityEnded`s, a `machine`
/// other than "deleted" is dropped, extra fields are dropped.
pub async fn thread_deleted(client: &dyn HostClient, thread_id: &str, o: &ThreadDeletedAsk) -> Result<ThreadDeleted, HostError> {
    let mut params = Frame::new();
    params.insert("threadId".into(), json!(thread_id));
    if let Some(processes) = &o.processes {
        params.insert("processes".into(), serde_json::to_value(processes).expect("processes write as JSON"));
    }
    if let Some(check) = o.check {
        params.insert("check".into(), json!(check));
    }
    let frame = client.request("sessions.delete", params).await?;
    let refused = || HostError::new(other_version("sessions.delete"));
    let ended = match frame.get("ended") {
        None => None,
        Some(Value::Array(items)) => {
            Some(items.iter().map(|item| parse::<ActivityEnded>(item).map_err(|_| refused())).collect::<Result<Vec<_>, _>>()?)
        }
        Some(_) => return Err(refused()),
    };
    Ok(ThreadDeleted {
        workspace_id: reply_field(&frame, "sessions.delete", "workspaceId")?,
        worktree: frame.get("worktree").and_then(Value::as_str).map(str::to_owned),
        threads: reply_field(&frame, "sessions.delete", "threads")?,
        ended,
        machine: (frame.get("machine").and_then(Value::as_str) == Some("deleted")).then_some(MachineDeleted::Deleted),
        on: frame.get("on").and_then(Value::as_str).map(str::to_owned),
    })
}

/// What `subFor` asks (turns-help.ts:320): `on`, then the optional size, base, branch and harness, and the project.
#[derive(Clone, Debug, Default, Deserialize)]
pub struct SubAsk {
    pub on: String,
    pub size: Option<String>,
    pub from: Option<String>,
    pub branch: Option<String>,
    pub harness: Option<String>,
    pub project: Option<String>,
}

/// What `subFor` answers: the machine's workspace, the sub-thread's start, and the call that ends the listening.
pub struct SubMade {
    pub workspace: WorkspaceOut,
    pub sub: SubStarted,
    pub stop: Box<dyn Fn() + Send + Sync>,
}

/// The names of the workspaces the host lists, as a set in the listing's order.
async fn taken_names(client: &dyn HostClient) -> Result<IndexSet<String>, HostError> {
    Ok(workspaces(client).await?.into_iter().map(|w| w.name).collect())
}

/// The folder target's `project` read off a `ProjectView`: the whole object, kept in the order the host wrote it.
fn pick_of(project: &ProjectView) -> ProjectPick {
    serde_json::from_value(serde_json::to_value(project).expect("a project writes as JSON")).expect("a project reads back as a pick")
}

/// The three fields a folder target names its project by: `id`, `name` and `path`.
fn folder_project(id: String, name: String, path: String) -> ProjectPick {
    ProjectPick { id, name, computer: None, rest: Map::from_iter([("path".to_owned(), json!(path))]) }
}

/// The line a folder target says for a thread it opened, with the folder shortened against HOME where `here` says so.
fn opened_of(name: Option<String>, here: bool) -> Opened {
    Arc::new(move |thread_id: &str, folder: Option<&str>| match folder {
        None => format!("thread {thread_id}"),
        Some(folder) => {
            let shown = if here { home_shortened(folder, std::env::var("HOME").ok().as_deref()) } else { folder.to_owned() };
            thread_opened_line(thread_id, name.as_deref(), &shown)
        }
    })
}

/// A thread's environment as the typed map the token reader takes.
fn to_hash_map(env: &IndexMap<String, String>) -> std::collections::HashMap<String, String> {
    env.iter().map(|(key, value)| (key.clone(), value.clone())).collect()
}

/// A path's length in UTF-16 units, as JavaScript's `string.length` counts it.
fn path_length(path: &str) -> usize {
    path.encode_utf16().count()
}
