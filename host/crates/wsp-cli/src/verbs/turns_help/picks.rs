// SPDX-License-Identifier: AGPL-3.0-only
//! `turns-help.ts:1-237` and `421-519` at 65f6b89c7: what a start carries. Spec: `spec/turns-picks.md`.
//! The drivers in `tests/cli_c/drivers_turns_picks.rs` and the cases of
//! `tests/fixtures/cli_c/turns_picks.json` say what each one does.

use std::collections::HashMap;
use std::io::Read;
use std::path::{Component, Path, PathBuf};

use base64::Engine;
use indexmap::IndexMap;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use wsp_wire::types::attachments::{Attachment, AttachmentRecord, UNTYPED_FILE};
use wsp_wire::types::check::parse;
use wsp_wire::types::conversations::{ResumeAsk, COPY_ALONE_FIX, COPY_ALONE_LINE, RESUME_WHERE_FIX, RESUME_WHERE_LINE};
use wsp_wire::types::js_number::{JsInt, JsNumber};
use wsp_wire::types::thread_defaults::{AccessChoice, ThreadDefaults};
use wsp_wire::views::harness::{start_picks, HarnessCatalog, PickRefusal, StartPicks};
use wsp_wire::views::preferences::Preferences;
use wsp_wire::views::session::ThreadView;
use wsp_wire::views::session_events::{turn_token_of, NOTIFY_ME};
use wsp_wire::views::workspace_more::WorktreeMade;
use wsp_words::attachments::{files_refusal, image_type_of, not_a_file_line};
use wsp_words::exit::{usage_refusal, HostError};
use wsp_words::js::{js_trim, to_number};
use wsp_words::thread_defaults::{access_word_refusal, pick_refusal, PickError};
use wsp_words::words::computer::GUEST_NO_FILE_LINE;
use wsp_words::words::thread::{at_not_a_turn_line, no_adapter_line, EMPTY_MESSAGE_LINE};

use super::target::{thread_at_in, FolderHere, ForkTarget};
use crate::verbs::client::{absolute_folder, access_word_of, flag, flag_set, other_version, pick, thread_id_of, Flags, Frame, HostClient};
use crate::verbs::workspaces_help::thread_of;

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
    Ok(AskedPicks {
        model: picks.model.clone(),
        effort: picks.effort.clone(),
        access: picks.access.as_deref().map(access_word_of).transpose()?,
        fast: picks.fast,
    })
}

/// The picks a start carries, as the frame writes them: `picks_of`'s fields in their order.
fn picks_value(picks: &Picks) -> Result<Value, HostError> {
    Ok(serde_json::to_value(picks_of(picks)?).expect("the picks write as JSON"))
}

/// Writes the fields of a JSON object onto a frame, in the object's own order.
fn extend(frame: &mut Frame, fields: Value) {
    if let Value::Object(fields) = fields {
        frame.extend(fields);
    }
}

/// A host's answer read whole into the type a verb takes, or the other-version refusal the answer earns.
pub(super) fn reply_field<T: DeserializeOwned>(frame: &Frame, op: &str, key: &str) -> Result<T, HostError> {
    frame.get(key).cloned().and_then(|value| serde_json::from_value(value).ok()).ok_or_else(|| HostError::new(other_version(op)))
}

/// The file's first `IMAGE_HEAD_BYTES`, the rest of the head left zero where the file is shorter.
fn head_of(path: &Path) -> std::io::Result<[u8; IMAGE_HEAD_BYTES]> {
    let mut head = [0u8; IMAGE_HEAD_BYTES];
    let mut file = std::fs::File::open(path)?;
    let mut filled = 0;
    while filled < IMAGE_HEAD_BYTES {
        let n = file.read(&mut head[filled..])?;
        if n == 0 {
            break;
        }
        filled += n;
    }
    Ok(head)
}

/// A path as Node's `path.resolve` writes it: against this process's folder when relative, `.` and `..` read away.
fn resolve_path(given: &str) -> PathBuf {
    let joined =
        if Path::new(given).is_absolute() { PathBuf::from(given) } else { std::env::current_dir().unwrap_or_default().join(given) };
    let mut out = PathBuf::new();
    for part in joined.components() {
        match part {
            Component::ParentDir => {
                out.pop();
            }
            Component::CurDir => {}
            other => out.push(other.as_os_str()),
        }
    }
    out
}

fn attachment_json(file: &Attachment) -> Value {
    let mut out = serde_json::Map::new();
    out.insert("mediaType".into(), json!(file.media_type));
    if let Some(name) = &file.name {
        out.insert("name".into(), json!(name));
    }
    out.insert("bytes".into(), json!(file.bytes));
    Value::Object(out)
}

fn attachments_value(files: &[Attachment]) -> Value {
    Value::Array(files.iter().map(attachment_json).collect())
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
    if elsewhere && !paths.is_empty() {
        return Err(usage_refusal(GUEST_NO_FILE_LINE, "Name a file on the machine the line runs on, or none."));
    }
    let mut found = Vec::with_capacity(paths.len());
    for given in paths {
        let path = resolve_path(given);
        let Some(meta) = std::fs::metadata(&path).ok().filter(|meta| meta.is_file()) else {
            return Err(usage_refusal(&not_a_file_line(given), "Name a file that is already here."));
        };
        let head = head_of(&path).map_err(|e| HostError::new(e.to_string()))?;
        let record = AttachmentRecord {
            media_type: image_type_of(&head).unwrap_or_else(|| UNTYPED_FILE.to_owned()),
            bytes: JsInt(JsNumber(meta.len() as f64)),
            name: path.file_name().map(|name| name.to_string_lossy().into_owned()),
        };
        found.push((path, record));
    }
    let records: Vec<AttachmentRecord> = found.iter().map(|(_, record)| record.clone()).collect();
    if let Some(refused) = files_refusal(&records) {
        return Err(usage_refusal(&format!("{refused}."), "Drop that one and send the rest."));
    }
    found
        .into_iter()
        .map(|(path, record)| {
            let bytes = std::fs::read(&path).map_err(|e| HostError::new(e.to_string()))?;
            Ok(Attachment {
                media_type: record.media_type,
                bytes: base64::engine::general_purpose::STANDARD.encode(bytes),
                name: record.name,
            })
        })
        .collect()
}

/// `pickFlags(flags)` (turns-help.ts:184-186): `model`, `effort` and `access` where the flag is a string (an empty one
/// included; a switch or a list is no value), then `fast` where the flag is the switch `true`; always in that order.
pub fn pick_flags(flags: &Flags) -> Picks {
    Picks {
        model: flag(flags, "model").map(str::to_owned),
        effort: flag(flags, "effort").map(str::to_owned),
        access: flag(flags, "access").map(str::to_owned),
        fast: flag_set(flags, "fast"),
    }
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
pub async fn checked_start(
    client: &dyn HostClient,
    task: &str,
    harness: Option<&str>,
    picks: &Picks,
    workspace_id: Option<&str>,
    fork_of: Option<&str>,
) -> Result<(), HostError> {
    if js_trim(task).is_empty() {
        return Err(usage_refusal(EMPTY_MESSAGE_LINE, "Put it in quotes after the flags."));
    }
    let harnesses = harness_lists(client, workspace_id).await?;
    let agent = match (harness, fork_of) {
        (Some(agent), _) => Some(agent.to_owned()),
        (None, Some(project)) => project_defaults_of(client).await?.get(project).map(|d| d.agent.value.clone()),
        (None, None) => None,
    };
    let table = harnesses
        .iter()
        .find(|c| match &agent {
            Some(agent) => c.harness == *agent,
            None => c.is_default == Some(true),
        })
        .cloned();
    if let (None, Some(agent)) = (&table, harness) {
        let all = match workspace_id {
            None => harnesses,
            Some(_) => harness_lists(client, None).await?,
        };
        if all.iter().any(|c| c.harness == agent) {
            return Ok(());
        }
        let names: Vec<&str> = all.iter().map(|c| c.harness.as_str()).collect();
        return Err(usage_refusal(&no_adapter_line(agent, &names), "Name one of those with --agent."));
    }
    if workspace_id.is_none() && fork_of.is_none() {
        return Ok(());
    }
    let asked = picks_of(picks)?;
    let start =
        StartPicks { model: asked.model.clone(), effort: asked.effort.clone(), permission_mode: None, fast: asked.fast.then_some(true) };
    if let Err(refusal) = start_picks(table.as_ref(), &start, true, None) {
        return Err(pick_refusal(&pick_error_of(refusal), table.as_ref()));
    }
    match (asked.access, table.as_ref()) {
        (Some(access), Some(table)) => access_word_refusal(table, access).map_or(Ok(()), Err),
        _ => Ok(()),
    }
}

/// `harnesses.list`, with `{workspaceId}` where a workspace is named; the answer's `harnesses`.
async fn harness_lists(client: &dyn HostClient, workspace_id: Option<&str>) -> Result<Vec<HarnessCatalog>, HostError> {
    let params = pick(vec![("workspaceId", workspace_id.map(|id| json!(id)))]);
    let rows: Vec<Value> = reply_field(&client.request("harnesses.list", params).await?, "harnesses.list", "harnesses")?;
    rows.into_iter()
        .map(|row| serde_json::from_value(with_catalog_keys(row)).map_err(|_| HostError::new(other_version("harnesses.list"))))
        .collect()
}

/// A catalog row as the host listed it, given the keys the typed record requires and the row lacks: the TS reads only the
/// tables it checks a start against and takes the rest as the host wrote it.
fn with_catalog_keys(mut row: Value) -> Value {
    if let Some(object) = row.as_object_mut() {
        if !matches!(object.get("source").and_then(Value::as_str), Some("harness" | "table")) {
            object.insert("source".to_owned(), Value::from("harness"));
        }
        object.entry("version").or_insert(Value::Null);
        object.entry("contextWindows").or_insert_with(|| json!([]));
        for key in ["steers", "renames", "images"] {
            object.entry(key).or_insert(Value::Bool(false));
        }
    }
    row
}

/// `PickRefusal` as `pickRefusal` reads a thrown pick error.
fn pick_error_of(refusal: PickRefusal) -> PickError {
    PickError { message: refusal.message, kind: Some(refusal.kind.to_owned()), offered: refusal.offered.map(|n| n as _) }
}

/// `projectDefaultsOf(client)` (turns-help.ts:220): `projects.defaults`, its `defaults` as a map of project id to
/// `ThreadDefaults` in the host's order; each value written agent, model, effort, access whatever order the host wrote.
pub async fn project_defaults_of(client: &dyn HostClient) -> Result<IndexMap<String, ThreadDefaults>, HostError> {
    let frame = client.request("projects.defaults", Frame::new()).await?;
    let refused = || HostError::new(other_version("projects.defaults"));
    let defaults = frame.get("defaults").and_then(Value::as_object).ok_or_else(refused)?;
    defaults.iter().map(|(project, value)| parse::<ThreadDefaults>(value).map(|d| (project.clone(), d)).map_err(|_| refused())).collect()
}

/// `preferencesOf(client)` (turns-help.ts:225): `preferences.get`, its `preferences`, read as `Preferences` (a field the
/// host leaves out takes its default, one it does not know is dropped).
pub async fn preferences_of(client: &dyn HostClient) -> Result<Preferences, HostError> {
    let frame = client.request("preferences.get", Frame::new()).await?;
    let value = frame.get("preferences").cloned().unwrap_or(Value::Null);
    parse::<Preferences>(&value).map_err(|_| HostError::new(other_version("preferences.get")))
}

/// `worktreeFor(client, project, branch)` (turns-help.ts:423-425): `worktree.make {project, branch}` read as
/// `WorktreeMade`: path, branch, made, whatever order and extra fields the host answered with.
pub async fn worktree_for(client: &dyn HostClient, project: &str, branch: &str) -> Result<WorktreeMade, HostError> {
    let params = pick(vec![("project", Some(json!(project))), ("branch", Some(json!(branch)))]);
    let frame = client.request("worktree.make", params).await?;
    parse::<WorktreeMade>(&Value::Object(frame)).map_err(|_| HostError::new(other_version("worktree.make")))
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
    let cwd = if opts.resume.is_some() {
        None
    } else {
        absolute_folder(match at {
            OpeningAt::Here(here) => here.cwd.as_deref(),
            _ => opts.cwd.as_deref(),
        })?
    };
    let attachments = files_from(&opts.files, opts.elsewhere)?;
    let env: HashMap<String, String> = env.iter().map(|(key, value)| (key.clone(), value.clone())).collect();
    let turn_token = turn_token_of(&env);
    let mut frame = Frame::new();
    match at {
        OpeningAt::Workspace { id } => {
            frame.insert("workspaceId".into(), json!(id));
        }
        OpeningAt::Here(here) => {
            if let Some(project) = &here.project {
                frame.insert("project".into(), json!(project.id));
            }
            if let Some(branch) = &here.branch {
                frame.insert("branch".into(), json!(branch));
            }
        }
        OpeningAt::Fork(_) => {}
        OpeningAt::Forks(thread) => {
            frame.insert("fork".into(), serde_json::to_value(&thread.forks).expect("a thread fork writes as JSON"));
            if let Some(branch) = &thread.branch {
                frame.insert("branch".into(), json!(branch));
            }
        }
    }
    frame.insert("prompt".into(), json!(prompt));
    if let Some(cwd) = cwd {
        frame.insert("cwd".into(), json!(cwd));
    }
    if let Some(harness) = &opts.harness {
        frame.insert("harness".into(), json!(harness));
    }
    if let Some(notify) = &opts.notify {
        frame.insert("notify".into(), json!(notify));
    }
    if let Some(token) = turn_token {
        frame.insert("turnToken".into(), json!(token));
    }
    if let Some(title) = &opts.title {
        frame.insert("title".into(), json!(title));
    }
    if let Some(replaces) = &opts.replaces {
        frame.insert("replaces".into(), json!(replaces));
    }
    if !attachments.is_empty() {
        frame.insert("attachments".into(), attachments_value(&attachments));
    }
    if let Some(resume) = &opts.resume {
        frame.insert("resume".into(), serde_json::to_value(resume).expect("a resume writes as JSON"));
    }
    extend(&mut frame, picks_value(&opts.picks)?);
    Ok(frame)
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
pub async fn thread_fork(
    client: &dyn HostClient,
    r#ref: &str,
    at: Option<&AtWord>,
    branch: Option<&str>,
    task: &str,
    harness: Option<&str>,
    picks: &Picks,
    cloud_on: bool,
) -> Result<ThreadFork, HostError> {
    let count = at.map(|at| match at {
        AtWord::Word(word) => to_number(word),
        AtWord::Count(n) => *n,
    });
    if let Some(n) = count.filter(|n| !is_safe_count(*n)) {
        let said = match at {
            Some(AtWord::Word(word)) => word.clone(),
            Some(AtWord::Count(_)) | None => JsNumber(n).js_string(),
        };
        return Err(usage_refusal(&at_not_a_turn_line(&said), "Name one with --at 1 for the thread's first."));
    }
    let (thread, _) = thread_at_in(client, r#ref, "wsp run --fork", false, cloud_on).await?;
    checked_start(client, task, harness.or(Some(thread.harness.as_str())), picks, Some(&thread.workspace_id), None).await?;
    Ok(ThreadFork {
        forks: ThreadForks { thread_id: thread_id_of(&thread), at: count.map(|n| n as u64) },
        branch: branch.map(str::to_owned),
    })
}

/// A count `--at` may name: a safe integer from 1 (`Number.isSafeInteger` and `>= 1`).
fn is_safe_count(n: f64) -> bool {
    n.fract() == 0.0 && n.abs() <= 9_007_199_254_740_991.0 && n >= 1.0
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
    let Some(id) = id else {
        if copy {
            return Err(usage_refusal(COPY_ALONE_LINE, COPY_ALONE_FIX));
        }
        return Ok(None);
    };
    if r#where.beside.is_some() || r#where.branch.is_some() || r#where.cwd.is_some() {
        return Err(usage_refusal(RESUME_WHERE_LINE, RESUME_WHERE_FIX));
    }
    Ok(Some(ResumeAsk { id: id.to_owned(), copy: copy.then_some(true) }))
}

/// `replacedOf(client, ref)` (turns-help.ts:490-496): nothing for no ref (nothing sent); else the thread by id or prefix
/// (`thread_of`), `sessions.replaceable {threadId}`, and the thread's full id (`threadId`, else its row id).
pub async fn replaced_of(client: &dyn HostClient, r#ref: Option<&str>) -> Result<Option<String>, HostError> {
    let Some(r#ref) = r#ref else { return Ok(None) };
    let thread = thread_of(client, r#ref).await?;
    let thread_id = thread_id_of(&thread);
    client.request("sessions.replaceable", pick(vec![("threadId", Some(json!(thread_id)))])).await?;
    Ok(Some(thread_id))
}

/// `notifyOf(client, refs)` (turns-help.ts:500-511): nothing for no refs; else, in the order named, `NOTIFY_ME` ("me",
/// exact) as it is and every other ref as its thread's full id, each through its own `thread_of` (one `sessions.list`
/// per thread ref, none for "me"); the first bad ref fails and the rest are not read.
pub async fn notify_of(client: &dyn HostClient, refs: &[String]) -> Result<Option<Vec<String>>, HostError> {
    if refs.is_empty() {
        return Ok(None);
    }
    let mut named = Vec::with_capacity(refs.len());
    for r#ref in refs {
        if r#ref == NOTIFY_ME {
            named.push(NOTIFY_ME.to_owned());
        } else {
            named.push(thread_id_of(&thread_of(client, r#ref).await?));
        }
    }
    Ok(Some(named))
}

/// `messageTo(thread, prompt, picks, files, elsewhere)` (turns-help.ts:516-519): the `sessions.start` fields of a message
/// to an existing thread: `workspaceId`, `prompt`, `harness` (the thread's), `thread` (its `threadId`, else its row id),
/// `attachments` (only with files; `files_from`), then the picks as `picks_of` writes them. The files are read before the
/// picks are checked; no folder, no target, no token.
pub fn message_to(thread: &ThreadView, prompt: &str, picks: &Picks, files: &[String], elsewhere: bool) -> Result<Frame, HostError> {
    let attachments = files_from(files, elsewhere)?;
    let mut frame = Frame::new();
    frame.insert("workspaceId".into(), json!(thread.workspace_id));
    frame.insert("prompt".into(), json!(prompt));
    frame.insert("harness".into(), json!(thread.harness));
    frame.insert("thread".into(), json!(thread_id_of(thread)));
    if !attachments.is_empty() {
        frame.insert("attachments".into(), attachments_value(&attachments));
    }
    extend(&mut frame, picks_value(picks)?);
    Ok(frame)
}
