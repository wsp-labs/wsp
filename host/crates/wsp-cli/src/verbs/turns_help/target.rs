// SPDX-License-Identifier: AGPL-3.0-only
//! `turns-help.ts:238-420` at 65f6b89c7: where a run goes and the threads a line names. Spec: `spec/turns-target.md`.
//! Every body is `todo!`; the drivers in `tests/cli_c/drivers_turns_target.rs` and the cases of
//! `tests/fixtures/cli_c/turns_target.json` say what each one does. `thread_at` is the signature other units call (fixed by
//! the lead); the cloud flag the TS reads from `CLOUD_ON` is a parameter on the helpers that read it, so a test can run
//! both settings in one process (`thread_at` itself reads `crate::cloud::cloud_on()` and is `thread_at_in` with it).

use std::sync::Arc;

use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use wsp_wire::types::sub_thread::SubStarted;
use wsp_wire::views::activity::{ActivityEnded, ProcessesAsk};
use wsp_wire::views::session::ThreadView;
use wsp_wire::views::workspace::ProjectView;
use wsp_wire::views::workspace_more::WorkspaceOut;
use wsp_words::exit::HostError;

use crate::verbs::client::HostClient;

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
    let _ = (client, r#ref, caller_cwd, env, elsewhere, r#where, cloud_on);
    todo!("turns-help.ts:247")
}

/// `threadsInFolder(client, project)` (turns-help.ts:296-299): whether `workspaces.landing {project}` answers a kind that
/// runs in a folder; a refused answer or one with no kind is false.
#[allow(dead_code)]
pub(crate) async fn threads_in_folder(client: &dyn HostClient, project: &str) -> bool {
    let _ = (client, project);
    todo!("turns-help.ts:296")
}

/// `forkFor(client, project, task, say)` (turns-help.ts:304-313): a machine forked for a run on a project elsewhere:
/// `workspaces.list` for the names taken, then `create_for(project, takenNameAfter(nameOfTask(task), taken))`, which sends
/// `workspaces.landing`, `events.subscribe` and `workspaces.create {project, name}`. `say` hears each `workspace.creating`
/// stage of that name as `message` trimmed at its end, then the created line (with the notice after a newline).
pub async fn fork_for(client: &dyn HostClient, project: &ProjectPick, task: &str, say: &(dyn Fn(&str) + Send + Sync)) -> Result<WorkspaceOut, HostError> {
    let _ = (client, project, task, say);
    todo!("turns-help.ts:304")
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

/// `subFor(client, ask, task, title, say)` (turns-help.ts:320-337): a sub-thread's machine on a cloud. The name is `title`
/// (not cut, an empty one is a name) else `name_of_task(task)`, numbered by `taken_name_after` against `workspaces.list`.
/// Then `events.subscribe`, and `workspaces.create {project: ask.project or "", name, sub: {on, size?, from?, branch?,
/// harness?}}`; each `workspace.creating` frame of that name, heard while the create is under way, after it and until `stop`
/// (also one heard during the subscribe), is said as its `message`, whole; the answer's `notice` is said a line at a time
/// (`split('\n')`) after the answer; `sub` is read as `SubStarted`. On a refusal the listening ends first.
pub async fn sub_for(client: &dyn HostClient, ask: &SubAsk, task: &str, title: Option<&str>, say: &(dyn Fn(&str) + Send + Sync)) -> Result<SubMade, HostError> {
    let _ = (client, ask, task, title, say);
    todo!("turns-help.ts:320")
}

/// `projectOfFolder(projects, folder)` (turns-help.ts:341-347): of the projects on this computer (`copies_folder` of
/// `kind_for_computer`), the one whose path holds the folder (`under`), the deepest (longest path) where two do, the first
/// listed on a tie; else the one whose `git.top` is the main folder of the repository the folder is in
/// (`main_worktree_of`, which reads the disk: the nearest `.git` up from the folder, a directory or a worktree's file
/// pointing under `<main>/.git/worktrees/`).
pub fn project_of_folder(projects: &[ProjectView], folder: &str) -> Option<ProjectView> {
    let _ = (projects, folder);
    todo!("turns-help.ts:341")
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
    let _ = (client, r#ref);
    todo!("turns-help.ts:351")
}

/// What `subDeleting` answers (turns-help.ts:361).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SubDeleting {
    pub id: String,
    pub workspace_id: String,
    pub question: String,
}

/// `subDeleting(client, thread)` (turns-help.ts:361-366): nothing for no thread or one with no `sub` (nothing sent); else
/// `sessions.delete {threadId, check: true}` and its `question`, with the thread's full id and workspace.
pub async fn sub_deleting(client: &dyn HostClient, thread: Option<&ThreadView>) -> Result<Option<SubDeleting>, HostError> {
    let _ = (client, thread);
    todo!("turns-help.ts:361")
}

/// `refuseMachineThread(client, ref)` (turns-help.ts:370-375): nothing for a ref that names no thread (or several, or a
/// refused listing); a thread whose workspace (`workspaces.list`, its refusal is the answer) does not run in a folder is
/// refused `thread_on_machine_line(workspace name)`, kind `usage` and no fix; a thread whose workspace is not listed is let by.
pub async fn refuse_machine_thread(client: &dyn HostClient, r#ref: &str) -> Result<(), HostError> {
    let _ = (client, r#ref);
    todo!("turns-help.ts:370")
}

/// `threadAt(client, ref, line, orComputer)` (turns-help.ts:381): the thread a ref names (`thread_of`), then its
/// workspace through `workspaces.get {workspaceId, threadId}` read as `WorkspaceOut` (another shape is
/// `other_version("workspaces.get")`); a ref naming no thread (kind `not-found` only) is read once more as a project
/// (or, with the cloud, a machine) and refused as `not_a_thread_line`. Reads the cloud setting as `cloud_on()`.
pub async fn thread_at(client: &dyn HostClient, r#ref: &str, line: &str, or_computer: bool) -> Result<(ThreadView, WorkspaceOut), HostError> {
    let _ = (client, r#ref, line, or_computer);
    todo!("turns-help.ts:381")
}

/// `thread_at` with the cloud setting given (what the TS reads off `CLOUD_ON`).
pub async fn thread_at_in(client: &dyn HostClient, r#ref: &str, line: &str, or_computer: bool, cloud_on: bool) -> Result<(ThreadView, WorkspaceOut), HostError> {
    let _ = (client, r#ref, line, or_computer, cloud_on);
    todo!("turns-help.ts:381")
}

/// `childOf(client, lead, ref, line)` (turns-help.ts:396-400): `thread_at_in(ref, line, false)`, and a child in the lead's
/// own workspace is refused `child_beside_lead_line(child id, lead id)` + `CHILD_BESIDE_LEAD_FIX` (usage).
pub async fn child_of(client: &dyn HostClient, lead: &(ThreadView, WorkspaceOut), r#ref: &str, line: &str, cloud_on: bool) -> Result<(ThreadView, WorkspaceOut), HostError> {
    let _ = (client, lead, r#ref, line, cloud_on);
    todo!("turns-help.ts:396")
}

/// `refuseNotAThread(client, ref, line, orComputer)` (turns-help.ts:404-408): a ref that is the id or name of a project
/// (`projects_here`, an unreadable list is none) is refused `not_a_thread_line(ref, project, line, orComputer)` +
/// `NAME_A_THREAD_FIX`; with the cloud, one that is the id or name of a workspace that does not run in a folder (a read
/// `workspaces.list` that failed is none) is refused the same as a machine; anything else is let by.
#[allow(dead_code)]
pub(crate) async fn refuse_not_a_thread(client: &dyn HostClient, r#ref: &str, line: &str, or_computer: bool, cloud_on: bool) -> Result<(), HostError> {
    let _ = (client, r#ref, line, or_computer, cloud_on);
    todo!("turns-help.ts:404")
}

/// `sharingWith(client, at)` (turns-help.ts:411-414): `sessions.list {workspaceId}` of the workspace, the thread ids of
/// the threads in it (folded as `threads` does), the host's order, minus the thread asking.
pub async fn sharing_with(client: &dyn HostClient, at: &(ThreadView, WorkspaceOut)) -> Result<Vec<String>, HostError> {
    let _ = (client, at);
    todo!("turns-help.ts:411")
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
    let _ = (client, thread_id, o);
    todo!("turns-help.ts:417")
}
