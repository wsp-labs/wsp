// SPDX-License-Identifier: AGPL-3.0-only
//! `workspaces-help.ts:602-881` at 65f6b89c7: dropping and deleting a workspace, the thread rows and tree, projects,
//! creating a workspace, sizes. Spec: `spec/ws-rows.md`. The first signatures are the ones other units call (fixed by the lead).
//! `branchHere` (workspaces-help.ts:871) is another unit's.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use wsp_wire::text::json_stringify;
use wsp_wire::types::workspace_state::UNNAMED_COMPUTER;
use wsp_wire::views::place_kind::PlaceKind;
use wsp_wire::views::session::{SessionStatus, ThreadView};
use wsp_wire::views::snapshot_lineage::WorkspaceCreateResult;
use wsp_wire::views::sub_row::SubState;
use wsp_wire::views::workspace::ProjectView;
use wsp_wire::views::workspace_more::{WorkspaceOut, WorkspaceView, WorktreeFolder};
use wsp_wire::wire::capabilities::{MachineSizeOffer, WorkspaceSize};
use wsp_words::exit::{usage_refusal, HostError};
use wsp_words::js::{number_string, to_number, utf16};
use wsp_words::place_name::table_name;
use wsp_words::projects::{
    computer_named, copies_folder, copy_takes_none, home_shortened, kind_for_computer, local_folder_refusal, local_worktree_refusal,
    runs_in_folder, source_word,
};
use wsp_words::thread_state::{subagent_state_word, thread_state, thread_word_of, ThreadState};
use wsp_words::words::base::fmt_threads;
use wsp_words::words::computer::Platform;
use wsp_words::words::image::forget_notice;
use wsp_words::words::units::{disk_expired, offered_size, size_from_word, size_refusal, DiskKeep, DISK_EXPIRED_WORD};
use wsp_words::workspace_state::{delete_notice, is_local_workspace, machine_word, on_delete_of, workspace_kind, CopyAt, StandsOn};

use super::reads::{params, place_ref, reply_field, shown, threads, workspace_of, workspace_typed, workspaces, ThreadRow};
use crate::verbs::client::{
    absolute_path, flag_for, host_platform, other_version, pick, places_of, Frame, HostClient, Out, PushedFrames, SshAsked,
};
use crate::verbs::io::Created;

fn wire_word<T: Serialize>(value: &T) -> String {
    match serde_json::to_value(value) {
        Ok(Value::String(word)) => word,
        _ => String::new(),
    }
}

fn platform() -> Platform {
    if host_platform() == "darwin" {
        Platform::Darwin
    } else {
        Platform::Linux
    }
}

#[derive(Deserialize)]
struct LandingSizes {
    sizes: Vec<MachineSizeOffer>,
}

async fn create_follow(
    client: &dyn HostClient,
    out: &Out,
    project: &ProjectPick,
    name: &str,
    asked: &CreateAsked,
    chosen: Option<WorkspaceSize>,
    pushed: &PushedFrames,
) -> Result<Created, HostError> {
    client.events().await?;
    let watched = name.to_owned();
    let emit_out = out.clone();
    let stream_out = out.clone();
    pushed.follow(
        Arc::new(move |f: &Frame| {
            f.get("type").and_then(Value::as_str) == Some("workspace.creating")
                && f.get("name").and_then(Value::as_str) == Some(watched.as_str())
        }),
        Arc::new(move |f: &Frame| {
            emit_out.emit(&Value::Object(f.clone()), None);
            let message = f.get("message").and_then(Value::as_str).unwrap_or_default();
            stream_out.stream(&format!("{message}\n"));
        }),
    );
    let mut body = pick(vec![("project", Some(json!(project.id))), ("name", Some(json!(name)))]);
    if let Some(size) = &chosen {
        body.insert("cpu".to_owned(), json!(size.cpu));
        body.insert("memMb".to_owned(), json!(size.mem_mb));
    }
    if let Some(agents) = &asked.agents {
        body.insert("agents".to_owned(), json!(agents));
    }
    if let Some(parent) = &asked.parent {
        body.insert("parent".to_owned(), json!(parent));
    }
    let reply = client.request("workspaces.create", body).await?;
    let shown_row: Value = reply_field(&reply, "workspaces.create", "workspace")?;
    let workspace = workspace_typed(shown_row.clone(), "workspaces.create")?;
    let notice = reply.get("notice").and_then(Value::as_str).map(str::to_owned);
    let at = match shown_row.get("folder").filter(|folder| !folder.is_null()) {
        Some(folder) => shown(Some(folder)),
        None => shown(shown_row.get("project").and_then(|project| project.get("path"))),
    };
    let line = format!(
        "created {} {}, a copy of {} at {}{}",
        shown(shown_row.get("name")),
        shown(shown_row.get("id")),
        shown(shown_row.get("project").and_then(|project| project.get("name"))),
        at,
        notice.as_ref().map(|n| format!("\n{n}")).unwrap_or_default()
    );
    let created = Created { workspace, shown: Some(shown_row), notice };
    out.emit(&serde_json::to_value(&created).unwrap_or(Value::Null), Some(&line));
    Ok(created)
}

// ---------------------------------------------------------------------------------------------------------------
// Dropping a workspace (workspaces-help.ts:600-666).
// ---------------------------------------------------------------------------------------------------------------

/// `madeWorktree(w)` (workspaces-help.ts:604): the worktree a delete takes with the record, only one wsp made
/// (`made` true) and has not removed (`gone` not true); its path as given. None for no worktree, a worktree wsp did
/// not make, and a made one that is gone.
pub fn made_worktree(worktree: Option<&WorktreeFolder>) -> Option<CopyAt> {
    worktree.filter(|w| w.made && w.gone.is_none()).map(|w| CopyAt { path: w.path.clone() })
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
fn on_delete(d: &Dropping) -> wsp_words::workspace_state::MachineOnDelete {
    on_delete_of(
        &workspace_kind(d.workspace.kind.as_ref()),
        made_worktree(d.workspace.worktree.as_ref()).as_ref(),
        Some(&d.workspace.machine_id),
        d.on.as_ref(),
    )
}

/// `dropping(client, ref)` (workspaces-help.ts:615-622): `workspace_of(ref)` (one `workspaces.resolve {ref}`; a reply
/// that does not read as a workspace is `other_version("workspaces.resolve")`), then in this order: a local workspace
/// with a worktree record, made or not, gone or not, is a usage refusal `local_worktree_refusal(name)` with the fix
/// `Remove it with wsp worktree remove <project> <branch>, or delete a thread there with wsp delete <thread>.`; a workspace
/// whose kind runs in a folder (local and place) is a usage refusal `local_folder_refusal(name)` with the fix `Run wsp
/// threads to find its threads, then wsp delete <thread>.`; both refusals come before any `sessions.list`. Else one
/// `sessions.list {workspaceId}` folded by `fold_threads`, its length the thread count. A record with no kind is a cloud one.
pub async fn dropping(client: &dyn HostClient, r#ref: &str) -> Result<Dropping, HostError> {
    let workspace = workspace_of(client, r#ref).await?;
    if is_local_workspace(workspace.kind.as_ref()) && workspace.worktree.is_some() {
        return Err(usage_refusal(
            &local_worktree_refusal(&workspace.name),
            "Remove it with wsp worktree remove <project> <branch>, or delete a thread there with wsp delete <thread>.",
        ));
    }
    if runs_in_folder(&workspace_kind(workspace.kind.as_ref())) {
        return Err(usage_refusal(
            &local_folder_refusal(&workspace.name),
            "Run wsp threads to find its threads, then wsp delete <thread>.",
        ));
    }
    let count = threads(client, Some(&workspace.id)).await?.len();
    Ok(Dropping { workspace, threads: count, on: None })
}

/// `deleting(client, ref)` (workspaces-help.ts:626-638): `dropping`, then, only where the workspace's record names a
/// `place`, one `places.list` (no params). A refused list names the computer `UNNAMED_COMPUTER` (`on` is set). A place
/// the list does not hold, or one of kind `provider`, leaves `on` unset (the delete takes its kind's words). Any other
/// place sets `on` to `{name: workspace name, computer: table_name(place)}`; the first row with the id is the place. A
/// refusal of `dropping` comes before any `places.list`.
pub async fn deleting(client: &dyn HostClient, r#ref: &str) -> Result<Dropping, HostError> {
    let mut d = dropping(client, r#ref).await?;
    let Some(at) = d.workspace.place.clone() else { return Ok(d) };
    let listed = match client.request("places.list", Frame::new()).await {
        Ok(reply) => Some(places_of(&reply, "places.list")?.into_iter().find(|p| p.id == at)),
        Err(_) => None,
    };
    let computer = match listed {
        Some(None) => return Ok(d),
        Some(Some(p)) if matches!(p.kind, PlaceKind::Provider) => return Ok(d),
        Some(Some(p)) => table_name(&place_ref(&p)),
        None => UNNAMED_COMPUTER.to_owned(),
    };
    d.on = Some(StandsOn { name: d.workspace.name.clone(), computer });
    Ok(d)
}

/// `forgetQuestion(f)` (workspaces-help.ts:641-643): `Forget <name>?`, a newline, `forget_notice(threads)`.
pub fn forget_question(f: &Dropping) -> String {
    format!("Forget {}?\n{}", f.workspace.name, forget_notice(f.threads as f64))
}

/// `forget(client, f)` (workspaces-help.ts:646-648): one `workspaces.forget {workspaceId}`; the runtime refuses while
/// its machine still exists. Answers nothing.
pub async fn forget(client: &dyn HostClient, f: &Dropping) -> Result<(), HostError> {
    client.request("workspaces.forget", params(json!({ "workspaceId": f.workspace.id }))).await?;
    Ok(())
}

/// `forgotLine(f)` (workspaces-help.ts:650-652): `forgot <name> <id>: its record and <fmt_threads> are gone from this computer`.
pub fn forgot_line(f: &Dropping) -> String {
    format!("forgot {} {}: its record and {} are gone from this computer", f.workspace.name, f.workspace.id, fmt_threads(f.threads as f64))
}

/// `deleteQuestion(d)` (workspaces-help.ts:655-657): `Delete <name>?`, a newline, `delete_notice(threads, kind,
/// made_worktree, machine id, on)`.
pub fn delete_question(d: &Dropping) -> String {
    let on = d.on.as_ref();
    let copy = made_worktree(d.workspace.worktree.as_ref());
    format!(
        "Delete {}?\n{}",
        d.workspace.name,
        delete_notice(d.threads as f64, &workspace_kind(d.workspace.kind.as_ref()), copy.as_ref(), Some(&d.workspace.machine_id), on)
    )
}

/// `deleteWorkspace(client, d)` (workspaces-help.ts:660-662): one `workspaces.delete {workspaceId}`. A machine already
/// gone is no error (the host's word, not this function's). Answers nothing.
pub async fn delete_workspace(client: &dyn HostClient, d: &Dropping) -> Result<(), HostError> {
    client.request("workspaces.delete", params(json!({ "workspaceId": d.workspace.id }))).await?;
    Ok(())
}

/// `deletedLine(d)` (workspaces-help.ts:664-666): `deleted <name> <id>: <on_delete.done(machine id)>, and its record and
/// <fmt_threads> are gone from this computer`.
pub fn deleted_line(d: &Dropping) -> String {
    format!(
        "deleted {} {}: {}, and its record and {} are gone from this computer",
        d.workspace.name,
        d.workspace.id,
        on_delete(d).done_for(&d.workspace.machine_id),
        fmt_threads(d.threads as f64)
    )
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
    let len = utf16::len(path);
    if len <= width {
        path.to_owned()
    } else {
        format!("…{}", utf16::slice(path, (len + 1 - width) as i64, None))
    }
}

/// `shortenedEnd(text, width)` (workspaces-help.ts:680-682): the text as it is when its length in UTF-16 units is at
/// most `width`, else its first `width - 1` units and `…`.
pub fn shortened_end(text: &str, width: usize) -> String {
    let len = utf16::len(text);
    if len <= width {
        text.to_owned()
    } else {
        format!("{}…", utf16::slice(text, 0, Some(width as i64 - 1)))
    }
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
    let thread = &t.thread;
    let folder = shortened_front(&home_shortened(&t.folder, home), FOLDER_WIDTH);
    let state = expired_word(t, now_ms).or_else(|| sub_thread_word(t)).map(str::to_owned).unwrap_or_else(|| thread_word_of(thread));
    let mut lines = vec![vec![
        t.project_name.clone(),
        folder.clone(),
        t.branch.clone(),
        format!("{indent}{}", thread.id),
        String::new(),
        thread.harness.clone(),
        state,
        wire_word(&thread.started_by),
        t.computer_name.clone(),
        shortened_end(&thread.title, TITLE_WIDTH),
    ]];
    for c in thread.subagents.iter().flatten() {
        lines.push(vec![
            t.project_name.clone(),
            folder.clone(),
            t.branch.clone(),
            format!("{indent}  {}", thread.id),
            c.id.clone(),
            thread.harness.clone(),
            subagent_state_word(c.state),
            "agent".to_owned(),
            t.computer_name.clone(),
            shortened_end(&c.title, TITLE_WIDTH),
        ]);
    }
    lines
}

/// `diskGoneLines(rows)` (workspaces-help.ts:696): one `<id>: <diskGone>` line per row that has a `diskGone` (an empty
/// string counts), in row order.
pub fn disk_gone_lines(rows: &[ThreadRow]) -> Vec<String> {
    rows.iter().filter_map(|t| t.thread.disk_gone.as_ref().map(|gone| format!("{}: {}", t.thread.id, gone))).collect()
}

/// `expiredWord(t)` (workspaces-help.ts:699-701): `DISK_EXPIRED_WORD` where nothing is waited on, the thread state is
/// not `failed` and `disk_expired(t, now)`; else none. Private: reached through `thread_lines`.
fn expired_word(t: &ThreadRow, now_ms: f64) -> Option<&'static str> {
    let thread = &t.thread;
    let status = wire_word(&thread.status);
    let keep =
        DiskKeep { disk_kept_until: thread.disk_kept_until.as_ref().map(|at| at.0), status: &status, asking: thread.asking.as_deref() };
    (thread.waiting_on.is_none() && thread_state(thread) != ThreadState::Failed && disk_expired(&keep, now_ms)).then_some(DISK_EXPIRED_WORD)
}

/// `subThreadWord(t)` (workspaces-help.ts:705-711): none for a row with no `sub`, one that asks, or one waited on. A sub
/// in state `waiting` is `Waiting`, `starting` is `Starting`, `released` is `Released` only while the thread state is
/// `completed` or `interrupted`; any other sub state is `Not pushed` when the turn is not running and work is held, else
/// none. Private: reached through `thread_lines`.
fn sub_thread_word(t: &ThreadRow) -> Option<&'static str> {
    let thread = &t.thread;
    let sub = thread.sub.as_ref()?;
    if thread.asking.is_some() || thread.waiting_on.is_some() {
        return None;
    }
    match sub.state {
        SubState::Waiting => Some("Waiting"),
        SubState::Starting => Some("Starting"),
        SubState::Released => matches!(thread_state(thread), ThreadState::Completed | ThreadState::Interrupted).then_some("Released"),
        _ => (!matches!(thread.status, SessionStatus::Running) && sub.held.is_some()).then_some("Not pushed"),
    }
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
    let held: HashSet<&str> = rows.iter().map(|r| r.thread.id.as_str()).collect();
    let mut under: HashMap<Option<&str>, Vec<&ThreadRow>> = HashMap::new();
    for row in rows {
        under.entry(row.thread.parent_thread_id.as_deref().filter(|p| held.contains(p))).or_default().push(row);
    }
    let mut drawn: HashSet<&str> = HashSet::new();
    let mut out = Vec::new();
    let mut stack = vec![under.get(&None).map_or([].iter(), |top| top.iter())];
    while let Some(level) = stack.last_mut() {
        let Some(&row) = level.next() else {
            stack.pop();
            continue;
        };
        let id = row.thread.id.as_str();
        if !drawn.insert(id) {
            continue;
        }
        out.push(TreeRow { row, depth: stack.len() - 1 });
        stack.push(under.get(&Some(id)).map_or([].iter(), |kids| kids.iter()));
    }
    for row in rows {
        if !drawn.contains(row.thread.id.as_str()) {
            out.push(TreeRow { row, depth: 0 });
        }
    }
    out
}

// ---------------------------------------------------------------------------------------------------------------
// Places and projects (workspaces-help.ts:736-770).
// ---------------------------------------------------------------------------------------------------------------

/// `placeNames(client)` (workspaces-help.ts:738-741): one `places.list`; each place's id to `table_name(place)`, in the
/// list's order. A repeated id keeps its first position and its last name (a JS `Map`).
pub async fn place_names(client: &dyn HostClient) -> Result<IndexMap<String, String>, HostError> {
    let reply = client.request("places.list", Frame::new()).await?;
    let places = places_of(&reply, "places.list")?;
    Ok(places.iter().map(|p| (p.id.clone(), table_name(&place_ref(p)))).collect())
}

/// `projectsOf(client)` (workspaces-help.ts:744): `projects.list`, its `projects` as the host wrote them.
pub async fn projects_listed(client: &dyn HostClient) -> Result<Vec<Value>, HostError> {
    let reply = client.request("projects.list", Frame::new()).await?;
    reply_field(&reply, "projects.list", "projects")
}

/// `projectOf(client, ref)` (workspaces-help.ts:749): `projects.resolve {ref}`, its `project` as the host wrote it.
pub async fn project_listed(client: &dyn HostClient, r#ref: &str) -> Result<Value, HostError> {
    let reply = client.request("projects.resolve", params(json!({ "ref": r#ref }))).await?;
    reply_field(&reply, "projects.resolve", "project")
}

const PROJECT_KEYS: [&str; 9] = ["id", "name", "computer", "path", "remote", "defaultBranch", "memoryKey", "memoryDir", "createdAt"];

/// A project row typed, the keys the record requires and the row lacks given as empty words.
pub fn project_typed(mut row: Value, op: &str) -> Result<ProjectView, HostError> {
    if let Some(object) = row.as_object_mut() {
        for key in PROJECT_KEYS {
            object.entry(key).or_insert_with(|| Value::from(""));
        }
        object.entry("source").or_insert_with(|| json!({ "kind": "folder", "path": "" }));
    }
    serde_json::from_value(row).map_err(|_| HostError::new(other_version(op)))
}

/// `projectsOf(client)` read into the typed record.
pub async fn projects_of(client: &dyn HostClient) -> Result<Vec<ProjectView>, HostError> {
    projects_listed(client).await?.into_iter().map(|row| project_typed(row, "projects.list")).collect()
}

/// `projectOf(client, ref)` read into the typed record.
pub async fn project_of(client: &dyn HostClient, r#ref: &str) -> Result<ProjectView, HostError> {
    project_typed(project_listed(client, r#ref).await?, "projects.resolve")
}

/// `projectLine(p, workspaces, threadsHeld, named)` (workspaces-help.ts:756-766): seven cells: name, id,
/// `computer_named(p.computer, named, host_platform())`, `source_word(source)`, the path shortened at `FOLDER_WIDTH`, the
/// base or an empty cell, and the count of threads whose workspace is one of `workspaces` of this project.
pub fn project_line(
    p: &ProjectView,
    workspaces: &[WorkspaceOut],
    threads_held: &[ThreadView],
    named: &IndexMap<String, String>,
) -> Vec<String> {
    project_row_line(&serde_json::to_value(p).unwrap_or(Value::Null), workspaces, threads_held, named)
}

/// `projectLine` over a row as the host wrote it: a cell the row lacks is empty.
pub fn project_row_line(
    p: &Value,
    workspaces: &[WorkspaceOut],
    threads_held: &[ThreadView],
    named: &IndexMap<String, String>,
) -> Vec<String> {
    let text = |key: &str| p.get(key).and_then(Value::as_str).unwrap_or_default().to_owned();
    let id = text("id");
    let source = p.get("source").and_then(|s| serde_json::from_value(s.clone()).ok()).map(|s| source_word(&s)).unwrap_or_default();
    let count = threads_held.iter().filter(|t| workspaces.iter().any(|w| w.id == t.workspace_id && w.project.id == id)).count();
    vec![
        text("name"),
        id,
        computer_named(&text("computer"), Some(named), &platform()),
        source,
        shortened_front(&text("path"), FOLDER_WIDTH),
        text("base"),
        count.to_string(),
    ]
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
///
/// Frames after the reply are dropped. The held frames stop on every road out once the create was sent.
pub async fn create_for(
    client: &dyn HostClient,
    out: &Out,
    project: &ProjectPick,
    name: &str,
    asked: &CreateAsked,
) -> Result<Created, HostError> {
    if copies_folder(&kind_for_computer(&project.computer, None)) && asked.size.is_some() {
        return Err(usage_refusal(&copy_takes_none(&project.name, &["--size"], None), "Drop them."));
    }
    let sized = asked.size.is_some().then(|| json!(true));
    let reply = client.request("workspaces.landing", pick(vec![("project", Some(json!(project.id))), ("sized", sized)])).await?;
    let chosen = match &asked.size {
        None => None,
        Some(word) => {
            let landing: LandingSizes = reply_field(&reply, "workspaces.landing", "capabilities")?;
            Some(size_chosen(&landing.sizes, word)?)
        }
    };
    let pushed = PushedFrames::new(client);
    let result = create_follow(client, out, project, name, asked, chosen, &pushed).await;
    pushed.stop();
    result
}

/// `projectsHere(client)` (workspaces-help.ts:817-823): `projects_of`; any refusal, or a reply with no `projects`, falls
/// back to `workspaces()` and the projects they hold, once each by project id (a repeated id keeps its first position
/// and its last name and computer). A list that is empty is an answer: no fallback read.
pub async fn projects_here(client: &dyn HostClient) -> Result<Vec<ProjectPick>, HostError> {
    if let Ok(held) = projects_listed(client).await {
        let text = |p: &Value, key: &str| p.get(key).and_then(Value::as_str).unwrap_or_default().to_owned();
        return Ok(held.iter().map(|p| ProjectPick { id: text(p, "id"), name: text(p, "name"), computer: text(p, "computer") }).collect());
    }
    let mut by_id: IndexMap<String, ProjectPick> = IndexMap::new();
    for w in workspaces(client).await? {
        by_id.insert(w.project.id.clone(), ProjectPick { id: w.project.id, name: w.project.name, computer: w.project.computer });
    }
    Ok(by_id.into_values().collect())
}

/// `agentsAsked(maxMachines, maxDepth, computer)` (workspaces-help.ts:827-832): `--max-machines` read before
/// `--max-depth`, each by `count_asked` (least 0 and 1); none when neither is named.
pub fn agents_asked(
    max_machines: Option<&Value>,
    max_depth: Option<&Value>,
    computer: Option<&str>,
) -> Result<Option<AgentsAsked>, HostError> {
    let machines = max_machines.map(|word| count_asked("--max-machines", word, 0, computer)).transpose()?;
    let depth = max_depth.map(|word| count_asked("--max-depth", word, 1, computer)).transpose()?;
    if machines.is_none() && depth.is_none() {
        return Ok(None);
    }
    Ok(Some(AgentsAsked { max_machines: machines, max_depth: depth }))
}

/// `countAsked(flagName, word, least, computer)` (workspaces-help.ts:834-838): `word` is a JSON string or number, read as
/// JS `Number(word)` does (white space trimmed, an empty or blank string is 0, `0x10` is 16, `1e2` is 100, `+3` is 3,
/// `Infinity` is no integer). It must be an integer not below `least`, else a usage refusal `<flag_for(flagName,
/// computer)> takes a whole number of zero|one or more, and got <JSON of the word as a string>.` with the fix `Write it
/// as <flagName> <n>.`
pub fn count_asked(flag_name: &str, word: &Value, least: u64, computer: Option<&str>) -> Result<u64, HostError> {
    let text = match word {
        Value::String(text) => text.clone(),
        other => number_string(other.as_f64().unwrap_or(f64::NAN)),
    };
    let n = match word {
        Value::String(text) => to_number(text),
        other => other.as_f64().unwrap_or(f64::NAN),
    };
    if !n.is_finite() || n.fract() != 0.0 || n < least as f64 {
        let got = json_stringify(&Value::String(text));
        let of = if least == 0 { "zero" } else { "one" };
        return Err(usage_refusal(
            &format!("{} takes a whole number of {of} or more, and got {got}.", flag_for(flag_name, computer)),
            &format!("Write it as {flag_name} <n>."),
        ));
    }
    Ok(n as u64)
}

/// `sizeChosen(sizes, word)` (workspaces-help.ts:843-847): `size_from_word(word)` (a trimmed `<cpus>x<gigabytes>`), which
/// must be one of the offered sizes (`offered_size`), else a usage refusal `size_refusal(word, sizes)` (the word as
/// given) with the fix `Name one of those with --size.`
pub fn size_chosen(sizes: &[MachineSizeOffer], word: &str) -> Result<WorkspaceSize, HostError> {
    let size = size_from_word(word);
    match size {
        Some(size) if offered_size(sizes, &size) => Ok(size),
        _ => Err(usage_refusal(&size_refusal(word, sizes), "Name one of those with --size.")),
    }
}

/// `sshAsked(name, port, keyPath)` (workspaces-help.ts:851-859): the port is read as JS `Number(port)` and must be an
/// integer from 1 to 65535, else a usage refusal `--ssh-port takes a port, and got <JSON of the word>.` with the fix `A
/// port is a whole number from 1 to 65535.`; the key goes through `absolute_path("--ssh-key is a path on this computer",
/// key)`. The port is read before the key. Fields in the order name, port, key; the absent ones left out.
pub fn ssh_asked(name: Option<&str>, port: Option<&str>, key_path: Option<&str>) -> Result<SshAsked, HostError> {
    let dialled = port.map(to_number);
    if let Some(d) = dialled {
        if !d.is_finite() || d.fract() != 0.0 || !(1.0..=65535.0).contains(&d) {
            let got = json_stringify(&Value::String(port.unwrap_or_default().to_owned()));
            return Err(usage_refusal(&format!("--ssh-port takes a port, and got {got}."), "A port is a whole number from 1 to 65535."));
        }
    }
    let key_path = match key_path {
        Some(key) => Some(absolute_path("--ssh-key is a path on this computer", key)?),
        None => None,
    };
    Ok(SshAsked { name: name.map(str::to_owned), port: dialled.map(|d| d as u16), key_path })
}

/// `createdExisting(out, workspace, notice)` (workspaces-help.ts:863-867): not exported by the TS, reached through the
/// verbs that record a machine. Emits `{workspace, notice?}` with the line `created <name> <id> (<machine_word(kind)>)` plus
/// `\n<notice>` where a notice is present.
pub fn created_existing(out: &Out, workspace: &WorkspaceView, notice: Option<&str>) -> WorkspaceCreateResult {
    let created = WorkspaceCreateResult { workspace: workspace.clone(), notice: notice.map(str::to_owned) };
    let kind = workspace_kind(workspace.kind.as_ref());
    let line = format!(
        "created {} {} ({}){}",
        workspace.name,
        workspace.id,
        machine_word(&kind),
        notice.map(|n| format!("\n{n}")).unwrap_or_default()
    );
    out.emit(&serde_json::to_value(&created).unwrap_or(Value::Null), Some(&line));
    created
}
