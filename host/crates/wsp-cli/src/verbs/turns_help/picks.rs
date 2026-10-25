// SPDX-License-Identifier: AGPL-3.0-only
//! `turns-help.ts:1-237` and `421-519` at 65f6b89c7: what a start carries. Spec: `spec/turns-picks.md`.
//! Every body is `todo!`; the drivers in `tests/cli_c/drivers_turns_picks.rs` and the cases of
//! `tests/fixtures/cli_c/turns_picks.json` say what each one does.

use indexmap::IndexMap;
use serde::{Deserialize, Serialize};

use wsp_wire::types::attachments::Attachment;
use wsp_wire::types::conversations::ResumeAsk;
use wsp_wire::types::thread_defaults::{AccessChoice, ThreadDefaults};
use wsp_wire::views::preferences::Preferences;
use wsp_wire::views::session::ThreadView;
use wsp_wire::views::workspace_more::WorktreeMade;
use wsp_words::exit::HostError;

use super::target::{FolderHere, ForkTarget};
use crate::verbs::client::{Flags, Frame, HostClient};

/// `Picks` (turns-help.ts:138): the model, effort and access a start names (`model`, `effort` and `access` are
/// `PICK_FLAGS`) and `fast`, as the composer's pickers name them. An empty string is a pick; only `fast == true` is kept.
#[derive(Clone, Debug, Default, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Picks {
    pub model: Option<String>,
    pub effort: Option<String>,
    pub access: Option<String>,
    #[serde(default)]
    pub fast: bool,
}

/// What `picksOf` answers (turns-help.ts:141): `Omit<StartPicks, "permissionMode"> & { access?: AccessChoice }`.
/// Written in the order model, effort, access, fast; an absent pick and `fast == false` are left out.
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub struct AskedPicks {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub effort: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub access: Option<AccessChoice>,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub fast: bool,
}

/// `picksOf(picks)` (turns-help.ts:141-148): the picks as `sessions.start` carries them. A given access word goes through
/// `access_word_of`, so a word that is none of ask, auto-edit, full and plan (a harness's own spelling, an empty string, a
/// different case) is the usage refusal `access_words_line` + `.`.
pub fn picks_of(picks: &Picks) -> Result<AskedPicks, HostError> {
    let _ = picks;
    todo!("turns-help.ts:141")
}

/// `IMAGE_HEAD_BYTES` (turns-help.ts:151): the head every image type is told apart by; a file shorter than this is
/// read as far as it goes and the rest of the head is zeros.
pub const IMAGE_HEAD_BYTES: usize = 12;

/// `filesFrom(paths, elsewhere)` (turns-help.ts:159-181): the files a `--file` flag or an MCP `files` list names, read off
/// this computer's disk, as `{ mediaType, name, bytes (base64) }`. With `elsewhere` and any path it refuses at once
/// (`guest_no_file_line`, fix "Name a file on the machine the line runs on, or none.") before a path is resolved; each
/// path is then taken to a file in turn (`not_a_file_line(given)`, fix "Name a file that is already here.", for a
/// missing path or a folder, the first one named), its first `IMAGE_HEAD_BYTES` read and told by `image_type_of`
/// (else `UNTYPED_FILE`), its name the path's last part, its size from the disk; then `files_refusal` (count over 5, an
/// empty file, an image over 10 MiB, another file over 10 MiB; the first thing wrong, in the order named), as `<it>.` with
/// the fix "Drop that one and send the rest."; only then is any file read whole. A relative path resolves against the
/// process's folder but is quoted as given. Each attachment is written `mediaType`, `name`, `bytes`.
pub fn files_from(paths: &[String], elsewhere: bool) -> Result<Vec<Attachment>, HostError> {
    let _ = (paths, elsewhere);
    todo!("turns-help.ts:159")
}

/// `pickFlags(flags)` (turns-help.ts:184-186): `model`, `effort` and `access` where the flag is a string (an empty one
/// included; a switch or a list is no value), then `fast` where the flag is the switch `true`; always in that order.
pub fn pick_flags(flags: &Flags) -> Picks {
    let _ = flags;
    todo!("turns-help.ts:184")
}

/// `checkedStart(client, task, harness, picks, workspaceId, forkOf)` (turns-help.ts:197-217): refuses in the runtime's
/// words, before a machine is made or woken, what the start would refuse there. In order:
/// 1. a task of only whitespace: `EMPTY_MESSAGE_LINE` with the fix "Put it in quotes after the flags." (no frame sent);
/// 2. `harnesses.list`, with `{workspaceId}` where one is named, else no params;
/// 3. the agent is `harness`, else with `forkOf` the `agent.value` that project holds in `projects.defaults` (read only then),
///    else none; its table is the row of that name, else (none named) the row marked `isDefault`;
/// 4. a named `harness` with no table: with a workspace `harnesses.list` is asked again, with no params; a list that
///    carries the agent passes (the start refuses it, naming that computer), else `no_adapter_line(harness, names)` with
///    the fix "Name one of those with --agent.";
/// 5. with neither a workspace nor `forkOf` nothing more is checked (the picks are the start's);
/// 6. `picks_of` (so a bad access word is refused first), `start_picks(table, asked, true)` refused through
///    `pick_refusal(e, table)`, then, for an access and a table, `access_word_refusal(table, access)`.
pub async fn checked_start(client: &dyn HostClient, task: &str, harness: Option<&str>, picks: &Picks, workspace_id: Option<&str>, fork_of: Option<&str>) -> Result<(), HostError> {
    let _ = (client, task, harness, picks, workspace_id, fork_of);
    todo!("turns-help.ts:197")
}

/// `projectDefaultsOf(client)` (turns-help.ts:220): `projects.defaults`, its `defaults` as a map of project id to
/// `ThreadDefaults` in the host's order; each value written agent, model, effort, access whatever order the host wrote.
pub async fn project_defaults_of(client: &dyn HostClient) -> Result<IndexMap<String, ThreadDefaults>, HostError> {
    let _ = client;
    todo!("turns-help.ts:220")
}

/// `preferencesOf(client)` (turns-help.ts:225): `preferences.get`, its `preferences`, read as `Preferences` (a field the
/// host leaves out takes its default, one it does not know is dropped).
pub async fn preferences_of(client: &dyn HostClient) -> Result<Preferences, HostError> {
    let _ = client;
    todo!("turns-help.ts:225")
}

/// `worktreeFor(client, project, branch)` (turns-help.ts:423-425): `worktree.make {project, branch}` read as
/// `WorktreeMade`: path, branch, made, whatever order and extra fields the host answered with.
pub async fn worktree_for(client: &dyn HostClient, project: &str, branch: &str) -> Result<WorktreeMade, HostError> {
    let _ = (client, project, branch);
    todo!("turns-help.ts:423")
}

/// What `openingOf` names the start's target by (turns-help.ts:434, its `at` argument): a workspace by id, a folder
/// target (`here`), a project's fork (`fork`, which names nothing in the frame) or a thread's fork (`forks`).
#[derive(Clone, Debug, PartialEq)]
pub enum OpeningAt {
    Workspace { id: String },
    Here(FolderHere),
    Fork(ForkTarget),
    Forks(ThreadFork),
}

/// The options of `openingOf` (turns-help.ts:434): the picks and what rides beside them. Absent is left out of the frame;
/// an empty string, an empty `notify` and an empty `title` are kept.
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OpeningOpts {
    #[serde(flatten)]
    pub picks: Picks,
    pub harness: Option<String>,
    pub cwd: Option<String>,
    pub notify: Option<Vec<String>>,
    pub title: Option<String>,
    pub replaces: Option<String>,
    #[serde(default)]
    pub files: Vec<String>,
    #[serde(default)]
    pub elsewhere: bool,
    pub resume: Option<ResumeAsk>,
}

/// `openingOf(env, at, prompt, opts)` (turns-help.ts:434-458): the `sessions.start` frame's fields, in this order:
/// the target (`workspaceId`; or for a folder target `project` (its id) then `branch`; nothing for a project's fork; for a
/// thread's fork `fork` then `branch`), `prompt`, `cwd`, `harness`, `notify`, `turnToken` (the `WSP_TURN` of `env`, read by
/// `turn_token_of`, absent when unset or empty), `title`, `replaces`, `attachments` (only with files), `resume`, then the
/// picks as `picks_of` writes them. `cwd` is the folder target's own `cwd` for a folder target (the options' is not read),
/// else the options'; it is left off by a `resume`, and a relative one is refused by `absolute_folder` (only where it is
/// used, so a resume never refuses one). The `cwd` refusal comes first, then the files (`files_from`), then the access
/// word.
pub fn opening_of(env: &IndexMap<String, String>, at: &OpeningAt, prompt: &str, opts: &OpeningOpts) -> Result<Frame, HostError> {
    let _ = (env, at, prompt, opts);
    todo!("turns-help.ts:434")
}

/// `ThreadFork` (turns-help.ts:462): a thread another one forks, as a start names it: `{ forks: { threadId, at? }, branch? }`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ThreadFork {
    pub forks: ThreadForks,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub branch: Option<String>,
}

/// The `forks` of a `ThreadFork`: the thread by its full id and the finished turn `--at` counts, from 1.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ThreadForks {
    pub thread_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub at: Option<u64>,
}

/// `--at` as the line or the tool gives it (turns-help.ts:469): a word, or a number.
#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(untagged)]
pub enum AtWord {
    Word(String),
    Count(f64),
}

/// `threadFork(client, ref, at, branch, task, harness, picks)` (turns-help.ts:469-475): the fork `--fork` names. `at` is
/// read as JavaScript's `Number()` reads a word (spaces around it, a sign, an exponent, `0x`, `0b`, `0o`, `3.`, `007`
/// are numbers; an empty word is 0; `abc`, `1_0`, `2nd` are not) and must be a safe integer of 1 or more, else the usage
/// refusal `at_not_a_turn_line(String(at))` with the fix "Name one with --at 1 for the thread's first." before any frame.
/// Then `thread_at_in(ref, "wsp run --fork", false, cloud_on)` (`cloud_on` is the TS's `CLOUD_ON`), then `checked_start(task, harness ?? thread.harness, picks, thread.workspaceId)`,
/// then `{ forks: { threadId (full id), at? }, branch? }`.
pub async fn thread_fork(client: &dyn HostClient, r#ref: &str, at: Option<&AtWord>, branch: Option<&str>, task: &str, harness: Option<&str>, picks: &Picks, cloud_on: bool) -> Result<ThreadFork, HostError> {
    let _ = (client, r#ref, at, branch, task, harness, picks, cloud_on);
    todo!("turns-help.ts:469")
}

/// Where a line asked the thread to run (`where` of `resumeAsked`, turns-help.ts:479): a word is named even when empty.
#[derive(Clone, Debug, Default, Deserialize)]
pub struct ResumeWhere {
    pub beside: Option<String>,
    pub branch: Option<String>,
    pub cwd: Option<String>,
}

/// `resumeAsked(id, copy, where)` (turns-help.ts:479-486): no conversation and no copy is nothing; `--copy` alone is the
/// usage refusal `COPY_ALONE_LINE` + `.` with `COPY_ALONE_FIX`; a conversation beside any of `beside`, `branch`, `cwd`
/// (an empty word counts) is `RESUME_WHERE_LINE` + `.` with `RESUME_WHERE_FIX`; else `{ id, copy: true? }`. No frame.
pub fn resume_asked(id: Option<&str>, copy: bool, r#where: &ResumeWhere) -> Result<Option<ResumeAsk>, HostError> {
    let _ = (id, copy, r#where);
    todo!("turns-help.ts:479")
}

/// `replacedOf(client, ref)` (turns-help.ts:490-496): nothing for no ref (nothing sent); else the thread by id or prefix
/// (`thread_of`), `sessions.replaceable {threadId}`, and the thread's full id (`threadId`, else its row id).
pub async fn replaced_of(client: &dyn HostClient, r#ref: Option<&str>) -> Result<Option<String>, HostError> {
    let _ = (client, r#ref);
    todo!("turns-help.ts:490")
}

/// `notifyOf(client, refs)` (turns-help.ts:500-511): nothing for no refs; else, in the order named, `NOTIFY_ME` ("me",
/// exact) as it is and every other ref as its thread's full id, each through its own `thread_of` (one `sessions.list`
/// per thread ref, none for "me"); the first bad ref fails and the rest are not read.
pub async fn notify_of(client: &dyn HostClient, refs: &[String]) -> Result<Option<Vec<String>>, HostError> {
    let _ = (client, refs);
    todo!("turns-help.ts:500")
}

/// `messageTo(thread, prompt, picks, files, elsewhere)` (turns-help.ts:516-519): the `sessions.start` fields of a message
/// to an existing thread: `workspaceId`, `prompt`, `harness` (the thread's), `thread` (its `threadId`, else its row id),
/// `attachments` (only with files; `files_from`), then the picks as `picks_of` writes them. The files are read before the
/// picks are checked; no folder, no target, no token.
pub fn message_to(thread: &ThreadView, prompt: &str, picks: &Picks, files: &[String], elsewhere: bool) -> Result<Frame, HostError> {
    let _ = (thread, prompt, picks, files, elsewhere);
    todo!("turns-help.ts:516")
}
