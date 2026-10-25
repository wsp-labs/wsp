// SPDX-License-Identifier: AGPL-3.0-only
//! `workspaces-help.ts:602-881` at 65f6b89c7: dropping and deleting a workspace, the thread rows and tree, projects,
//! creating a workspace, sizes. Spec: `spec/ws-rows.md`. The first signatures are the ones other units call (fixed by the lead).
//! `branchHere` (workspaces-help.ts:871) is another unit's.

use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use wsp_wire::views::session::ThreadView;
use wsp_wire::views::snapshot_lineage::WorkspaceCreateResult;
use wsp_wire::views::workspace::ProjectView;
use wsp_wire::views::workspace_more::{WorkspaceOut, WorkspaceView, WorktreeFolder};
use wsp_wire::wire::capabilities::{MachineSizeOffer, WorkspaceSize};
use wsp_words::exit::HostError;
use wsp_words::workspace_state::{CopyAt, StandsOn};

use super::reads::ThreadRow;
use crate::verbs::client::{HostClient, Out, SshAsked};
use crate::verbs::io::Created;

// ---------------------------------------------------------------------------------------------------------------
// Dropping a workspace (workspaces-help.ts:600-666).
// ---------------------------------------------------------------------------------------------------------------

/// `madeWorktree(w)` (workspaces-help.ts:604): the worktree a delete takes with the record, only one wsp made
/// (`made` true) and has not removed (`gone` not true); its path as given. None for no worktree, a worktree wsp did
/// not make, and a made one that is gone.
pub fn made_worktree(worktree: Option<&WorktreeFolder>) -> Option<CopyAt> {
    let _ = worktree;
    todo!("workspaces-help.ts:604")
}

/// `Dropping` (workspaces-help.ts:607-613): what dropping a workspace takes off this computer, counted before anyone is
/// asked. `on` is the computer somebody joined that it stands on, by the names a person reads; set by `deleting` alone.
#[derive(Clone, Debug, PartialEq)]
pub struct Dropping {
    pub workspace: WorkspaceOut,
    pub threads: usize,
    pub on: Option<StandsOn>,
}

/// `onDelete(d)` (workspaces-help.ts:602): `on_delete_of(kind of d.workspace, made_worktree, machine id, d.on)`. Private:
/// reached through `delete_question` and `deleted_line`.
#[allow(dead_code)]
fn on_delete(d: &Dropping) -> wsp_words::workspace_state::MachineOnDelete {
    let _ = d;
    todo!("workspaces-help.ts:602")
}

/// `dropping(client, ref)` (workspaces-help.ts:615-622): `workspace_of(ref)` (one `workspaces.resolve {ref}`; a reply
/// that does not read as a workspace is `other_version("workspaces.resolve")`), then in this order: a local workspace
/// with a worktree record, made or not, gone or not, is a usage refusal `local_worktree_refusal(name)` with the fix
/// `Remove it with wsp worktree remove <project> <branch>, or delete a thread there with wsp delete <thread>.`; a workspace
/// whose kind runs in a folder (local and place) is a usage refusal `local_folder_refusal(name)` with the fix `Run wsp
/// threads to find its threads, then wsp delete <thread>.`; both refusals come before any `sessions.list`. Else one
/// `sessions.list {workspaceId}` folded by `fold_threads`, its length the thread count. A record with no kind is a cloud one.
pub async fn dropping(client: &dyn HostClient, r#ref: &str) -> Result<Dropping, HostError> {
    let _ = (client, r#ref);
    todo!("workspaces-help.ts:615-622")
}

/// `deleting(client, ref)` (workspaces-help.ts:626-638): `dropping`, then, only where the workspace's record names a
/// `place`, one `places.list` (no params). A refused list names the computer `UNNAMED_COMPUTER` (`on` is set). A place
/// the list does not hold, or one of kind `provider`, leaves `on` unset (the delete takes its kind's words). Any other
/// place sets `on` to `{name: workspace name, computer: table_name(place)}`; the first row with the id is the place. A
/// refusal of `dropping` comes before any `places.list`.
pub async fn deleting(client: &dyn HostClient, r#ref: &str) -> Result<Dropping, HostError> {
    let _ = (client, r#ref);
    todo!("workspaces-help.ts:626-638")
}

/// `forgetQuestion(f)` (workspaces-help.ts:641-643): `Forget <name>?`, a newline, `forget_notice(threads)`.
pub fn forget_question(f: &Dropping) -> String {
    let _ = f;
    todo!("workspaces-help.ts:641-643")
}

/// `forget(client, f)` (workspaces-help.ts:646-648): one `workspaces.forget {workspaceId}`; the runtime refuses while
/// its machine still exists. Answers nothing.
pub async fn forget(client: &dyn HostClient, f: &Dropping) -> Result<(), HostError> {
    let _ = (client, f);
    todo!("workspaces-help.ts:646-648")
}

/// `forgotLine(f)` (workspaces-help.ts:650-652): `forgot <name> <id>: its record and <fmt_threads> are gone from this computer`.
pub fn forgot_line(f: &Dropping) -> String {
    let _ = f;
    todo!("workspaces-help.ts:650-652")
}

/// `deleteQuestion(d)` (workspaces-help.ts:655-657): `Delete <name>?`, a newline, `delete_notice(threads, kind,
/// made_worktree, machine id, on)`.
pub fn delete_question(d: &Dropping) -> String {
    let _ = d;
    todo!("workspaces-help.ts:655-657")
}

/// `deleteWorkspace(client, d)` (workspaces-help.ts:660-662): one `workspaces.delete {workspaceId}`. A machine already
/// gone is no error (the host's word, not this function's). Answers nothing.
pub async fn delete_workspace(client: &dyn HostClient, d: &Dropping) -> Result<(), HostError> {
    let _ = (client, d);
    todo!("workspaces-help.ts:660-662")
}

/// `deletedLine(d)` (workspaces-help.ts:664-666): `deleted <name> <id>: <on_delete.done(machine id)>, and its record and
/// <fmt_threads> are gone from this computer`.
pub fn deleted_line(d: &Dropping) -> String {
    let _ = d;
    todo!("workspaces-help.ts:664-666")
}

// ---------------------------------------------------------------------------------------------------------------
// The thread rows and tree (workspaces-help.ts:668-734).
// ---------------------------------------------------------------------------------------------------------------

/// `FOLDER_WIDTH` (workspaces-help.ts:669): the most UTF-16 units a folder cell holds before its front is cut.
pub const FOLDER_WIDTH: usize = 40;

/// `TITLE_WIDTH` (workspaces-help.ts:672): the most UTF-16 units a title cell holds before its end is cut.
pub const TITLE_WIDTH: usize = 60;

/// `shortenedFront(path, width)` (workspaces-help.ts:675-677): the path as it is when its length in UTF-16 units is at
/// most `width`, else `…` and the last `width - 1` units. Counts units, not characters: an emoji is two.
pub fn shortened_front(path: &str, width: usize) -> String {
    let _ = (path, width);
    todo!("workspaces-help.ts:675-677")
}

/// `shortenedEnd(text, width)` (workspaces-help.ts:680-682): the text as it is when its length in UTF-16 units is at
/// most `width`, else its first `width - 1` units and `…`.
pub fn shortened_end(text: &str, width: usize) -> String {
    let _ = (text, width);
    todo!("workspaces-help.ts:680-682")
}

/// `THREAD_HEAD` (workspaces-help.ts:684): the head of the threads table, ten columns.
pub const THREAD_HEAD: [&str; 10] = ["PROJECT", "FOLDER", "BRANCH", "THREAD", "SUBAGENT", "AGENT", "STATE", "BY", "COMPUTER", "TITLE"];

/// `threadLines(t, indent)` (workspaces-help.ts:688-693): a thread's row and one row per subagent under it. Own row:
/// project name, the folder (the home shortened to `~` where the folder is under it, then `shortened_front` at
/// `FOLDER_WIDTH`), branch, `<indent><id>`, an empty SUBAGENT cell, the agent id, the state word (first of: the
/// expired word, the sub-thread word, `thread_word_of`), `started_by`, the computer name, `shortened_end(title,
/// TITLE_WIDTH)`. A subagent row: the same project, folder and branch, `<indent>  <id>`, the subagent id, the agent id,
/// `subagent_state_word`, the literal `agent`, the computer name, its title shortened. `home` is the TS's `os.homedir()`
/// and `now_ms` its `Date.now()` (read by the expired word), both handed in so a test can pin them.
pub fn thread_lines(t: &ThreadRow, indent: &str, home: Option<&str>, now_ms: f64) -> Vec<Vec<String>> {
    let _ = (t, indent, home, now_ms);
    todo!("workspaces-help.ts:688-693")
}

/// `diskGoneLines(rows)` (workspaces-help.ts:696): one `<id>: <diskGone>` line per row that has a `diskGone` (an empty
/// string counts), in row order.
pub fn disk_gone_lines(rows: &[ThreadRow]) -> Vec<String> {
    let _ = rows;
    todo!("workspaces-help.ts:696")
}

/// `expiredWord(t)` (workspaces-help.ts:699-701): `DISK_EXPIRED_WORD` where nothing is waited on, the thread state is
/// not `failed` and `disk_expired(t, now)`; else none. Private: reached through `thread_lines`.
#[allow(dead_code)]
fn expired_word(t: &ThreadRow, now_ms: f64) -> Option<&'static str> {
    let _ = (t, now_ms);
    todo!("workspaces-help.ts:699-701")
}

/// `subThreadWord(t)` (workspaces-help.ts:705-711): none for a row with no `sub`, one that asks, or one waited on. A sub
/// in state `waiting` is `Waiting`, `starting` is `Starting`, `released` is `Released` only while the thread state is
/// `completed` or `interrupted`; any other sub state is `Not pushed` when the turn is not running and work is held, else
/// none. Private: reached through `thread_lines`.
#[allow(dead_code)]
fn sub_thread_word(t: &ThreadRow) -> Option<&'static str> {
    let _ = t;
    todo!("workspaces-help.ts:705-711")
}

/// One row of the `--tree` listing: the row (borrowed from the listing) and how many steps it is indented.
#[derive(Clone, Copy, Debug)]
pub struct TreeRow<'a> {
    pub row: &'a ThreadRow,
    pub depth: usize,
}

/// `threadTree(rows)` (workspaces-help.ts:716-734): depth first, in listing order at every level. A row whose
/// `parentThreadId` names a row of the listing stands under it; any other row (no parent, a parent not in the listing,
/// the empty string) is a top row. Rows left over (a parent that leads round in a circle, a row that is its own parent)
/// then stand flat at the top in listing order. A row whose id was already drawn is never drawn again, so the second of
/// two rows with one id is dropped.
pub fn thread_tree(rows: &[ThreadRow]) -> Vec<TreeRow<'_>> {
    let _ = rows;
    todo!("workspaces-help.ts:716-734")
}

// ---------------------------------------------------------------------------------------------------------------
// Places and projects (workspaces-help.ts:736-770).
// ---------------------------------------------------------------------------------------------------------------

/// `placeNames(client)` (workspaces-help.ts:738-741): one `places.list`; each place's id to `table_name(place)`, in the
/// list's order. A repeated id keeps its first position and its last name (a JS `Map`).
pub async fn place_names(client: &dyn HostClient) -> Result<IndexMap<String, String>, HostError> {
    let _ = client;
    todo!("workspaces-help.ts:738-741")
}

/// `projectsOf(client)` (workspaces-help.ts:744): `projects.list`, its `projects`.
pub async fn projects_of(client: &dyn HostClient) -> Result<Vec<ProjectView>, HostError> {
    let _ = client;
    todo!("workspaces-help.ts:744")
}

/// `projectOf(client, ref)` (workspaces-help.ts:749): `projects.resolve {ref}`, its `project`.
pub async fn project_of(client: &dyn HostClient, r#ref: &str) -> Result<ProjectView, HostError> {
    let _ = (client, r#ref);
    todo!("workspaces-help.ts:749")
}

/// `projectLine(p, workspaces, threadsHeld, named)` (workspaces-help.ts:756-766): seven cells: name, id,
/// `computer_named(p.computer, named, host_platform())`, `source_word(source)`, the path shortened at `FOLDER_WIDTH`, the
/// base or an empty cell, and the count of threads whose workspace is one of `workspaces` of this project.
pub fn project_line(p: &ProjectView, workspaces: &[WorkspaceOut], threads_held: &[ThreadView], named: &IndexMap<String, String>) -> Vec<String> {
    let _ = (p, workspaces, threads_held, named);
    todo!("workspaces-help.ts:756-766")
}

/// `NO_PROJECT_YET` (workspaces-help.ts:770).
pub const NO_PROJECT_YET: &str = "no projects yet; wsp add <folder> records one here, and wsp add <url> --on <computer> records one there";

// ---------------------------------------------------------------------------------------------------------------
// Creating a workspace (workspaces-help.ts:772-847).
// ---------------------------------------------------------------------------------------------------------------

/// `Pick<ProjectView, "id" | "name" | "computer">`: what `createFor` and `projectsHere` read of a project.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectPick {
    pub id: String,
    pub name: String,
    pub computer: String,
}

/// `Partial<WorkspaceAgents>` as `agentsAsked` builds it (workspaces-help.ts:780, 827-832): only the caps named, `maxMachines`
/// before `maxDepth`.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentsAsked {
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub max_machines: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub max_depth: Option<u64>,
}

/// The `asked` of `createFor` (workspaces-help.ts:780): the `--size` word, the caps and the parent workspace id.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CreateAsked {
    pub size: Option<String>,
    pub agents: Option<AgentsAsked>,
    pub parent: Option<String>,
}

/// `createFor(client, out, project, name, asked)` (workspaces-help.ts:775-812): a workspace of one project.
/// 1. A size asked for a project whose computer's kind copies a folder (`copies_folder(kind_for_computer(computer))`, so
///    a project on this computer) is a usage refusal `copy_takes_none(project name, ["--size"])`, fix `Drop them.`, before
///    any frame is sent (an empty size word counts as asked).
/// 2. `workspaces.landing {project: id, sized: true?}` (`sized` only when a size was asked); its `capabilities` is read
///    for `sizes` alone (the reply carries no more than that in the recorded cases, and `sizes` is read only with a size asked).
/// 3. With a size, `size_chosen(sizes, word)` (a refusal before `events.subscribe`).
/// 4. Frames held from here; `events.subscribe`; each `workspace.creating` frame whose `name` is this name (those held
///    first, in order, then live ones) goes `out.emit(frame)` then `out.stream("<message>\n")`.
/// 5. `workspaces.create {project, name, ...size (cpu, memMb), agents?, parent?}` in that key order.
/// 6. The reply's `workspace` and `notice` (kept as the host wrote them) are emitted with the line `created <name> <id>, a
///    copy of <project name> at <folder or project path>` plus `\n<notice>` where a notice is present (an empty one too).
/// Frames after the reply are dropped. The held frames stop on every road out once the create was sent.
pub async fn create_for(client: &dyn HostClient, out: &Out, project: &ProjectPick, name: &str, asked: &CreateAsked) -> Result<Created, HostError> {
    let _ = (client, out, project, name, asked);
    todo!("workspaces-help.ts:775-812")
}

/// `projectsHere(client)` (workspaces-help.ts:817-823): `projects_of`; any refusal, or a reply with no `projects`, falls
/// back to `workspaces()` and the projects they hold, once each by project id (a repeated id keeps its first position
/// and its last name and computer). A list that is empty is an answer: no fallback read.
pub async fn projects_here(client: &dyn HostClient) -> Result<Vec<ProjectPick>, HostError> {
    let _ = client;
    todo!("workspaces-help.ts:817-823")
}

/// `agentsAsked(maxMachines, maxDepth, computer)` (workspaces-help.ts:827-832): `--max-machines` read before
/// `--max-depth`, each by `count_asked` (least 0 and 1); none when neither is named.
pub fn agents_asked(max_machines: Option<&Value>, max_depth: Option<&Value>, computer: Option<&str>) -> Result<Option<AgentsAsked>, HostError> {
    let _ = (max_machines, max_depth, computer);
    todo!("workspaces-help.ts:827-832")
}

/// `countAsked(flagName, word, least, computer)` (workspaces-help.ts:834-838): `word` is a JSON string or number, read as
/// JS `Number(word)` does (white space trimmed, an empty or blank string is 0, `0x10` is 16, `1e2` is 100, `+3` is 3,
/// `Infinity` is no integer). It must be an integer not below `least`, else a usage refusal `<flag_for(flagName,
/// computer)> takes a whole number of zero|one or more, and got <JSON of the word as a string>.` with the fix `Write it
/// as <flagName> <n>.`
pub fn count_asked(flag_name: &str, word: &Value, least: u64, computer: Option<&str>) -> Result<u64, HostError> {
    let _ = (flag_name, word, least, computer);
    todo!("workspaces-help.ts:834-838")
}

/// `sizeChosen(sizes, word)` (workspaces-help.ts:843-847): `size_from_word(word)` (a trimmed `<cpus>x<gigabytes>`), which
/// must be one of the offered sizes (`offered_size`), else a usage refusal `size_refusal(word, sizes)` (the word as
/// given) with the fix `Name one of those with --size.`
pub fn size_chosen(sizes: &[MachineSizeOffer], word: &str) -> Result<WorkspaceSize, HostError> {
    let _ = (sizes, word);
    todo!("workspaces-help.ts:843-847")
}

/// `sshAsked(name, port, keyPath)` (workspaces-help.ts:851-859): the port is read as JS `Number(port)` and must be an
/// integer from 1 to 65535, else a usage refusal `--ssh-port takes a port, and got <JSON of the word>.` with the fix `A
/// port is a whole number from 1 to 65535.`; the key goes through `absolute_path("--ssh-key is a path on this computer",
/// key)`. The port is read before the key. Fields in the order name, port, key; the absent ones left out.
pub fn ssh_asked(name: Option<&str>, port: Option<&str>, key_path: Option<&str>) -> Result<SshAsked, HostError> {
    let _ = (name, port, key_path);
    todo!("workspaces-help.ts:851-859")
}

/// `createdExisting(out, workspace, notice)` (workspaces-help.ts:863-867): not exported by the TS, reached through the
/// verbs that record a machine. Emits `{workspace, notice?}` with the line `created <name> <id> (<machine_word(kind)>)` plus
/// `\n<notice>` where a notice is present.
pub fn created_existing(out: &Out, workspace: &WorkspaceView, notice: Option<&str>) -> WorkspaceCreateResult {
    let _ = (out, workspace, notice);
    todo!("workspaces-help.ts:863-867")
}
