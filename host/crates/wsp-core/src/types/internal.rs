// SPDX-License-Identifier: AGPL-3.0-only
//! Port of packages/runtime/src/types/internal.ts: the store's collection names, the records the runtime keeps, the
//! transcript index and its fold, the log lines, and the stand-in machines. Specs: spec/internal-records.md,
//! spec/internal-index.md, spec/internal-machines.md.

use std::cell::RefCell;
use std::collections::BTreeSet;
use std::fs;
use std::rc::Rc;
use std::sync::Arc;

use async_trait::async_trait;
use futures::StreamExt;
use sha2::{Digest, Sha256};

use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use wsp_wire::types::js_number::JsNumber;

use crate::node_path;
use wsp_adapters::port::{ExecStreamFactory, KeptAgent, SessionFile};
use wsp_engine::errors::{EngineError, ErrorKind, WspError};
use wsp_engine::labels::CREATED_AT_LABEL;
use wsp_engine::machine::{ExecOptions, Machine, MachineBackend, MachineLife, MachineState, RunOptions, SnapshotOptions};
use wsp_engine::no_provider_backend::NoProviderBackend;
use wsp_engine::Cancel;
use wsp_wire::types::agents_report::CONFIG_READ_MAX_BYTES;
use wsp_wire::types::attachments::{Attachment, AttachmentRecord};
use wsp_wire::types::check::parse;
use wsp_wire::types::thread_defaults::AccessChoice;
use wsp_wire::views::golden_image::{GoldenBaseTool, GoldenRetired, GoldenStage, GoldenStep, MachineKind};
use wsp_wire::views::harness::McpServerSpec;
use wsp_wire::views::project_bundle::{ProjectImportResult, ProjectImportStage};
use wsp_wire::views::session::{ForkedFrom, SessionOrigin, SessionView, ThreadPlacement};
use wsp_wire::views::session_events::{SessionEvent, TurnStatus};
use wsp_wire::views::workspace::ProjectView;
use wsp_wire::views::workspace_kind::WorkspaceKind;
use wsp_wire::views::workspace_more::{ThreadScope, WorkspaceOrigin, WorktreeFolder};
use wsp_wire::wire::capabilities::WorkspaceSize;
use wsp_wire::wire::daemon::EXEC_OUTPUT_MAX;
use wsp_wire::wire::machine_link::{ExecResult, MachineSpec, PreviewReach};
use wsp_words::exit::{refusal, HostError, Kind};
use wsp_words::js::{collate, utf16};
use wsp_words::words::computer::{default_branch_fix, default_branch_refusal, thread_word, DefaultBranchRoad};
use wsp_words::words::units::{fmt_bytes, fmt_duration, DurationStyle};

use super::pending::{Builder, ImportLedger, MachineExecOptions, ScopedRoad, TurnWaiting};
use super::wiring::{GoldenRecipe, LiveWorkspace, Local, ProjectImportOptions, SessionHandle, StageReport, WorkspaceRecord};

// --- collections ---------------------------------------------------------------------------------------------------

/// internal.ts:44.
pub const WORKSPACES: &str = "workspaces";
/// internal.ts:47.
pub const PROJECTS: &str = "projects";
/// internal.ts:50.
pub const SEED_CHOICES: &str = "seed-choices";
/// internal.ts:53.
pub const GOLDENS: &str = "goldens";
/// internal.ts:55.
pub const GOLDEN_RECIPES: &str = "golden-recipes";
/// internal.ts:57.
pub const PROJECT_GOLDENS: &str = "project-goldens";
/// internal.ts:59.
pub const IMAGES: &str = "images";
/// internal.ts:61.
pub const NO_COPY_RECIPE: &str = "this runtime cannot compose the recipe a copy builds from; the host that serves the app wires one";
/// internal.ts:64.
pub const NO_SEED_WIRING: &str = "this runtime cannot read a folder on this computer, so a folder cannot seed a project elsewhere; the host that serves the app wires the reader";
/// internal.ts:67.
pub const IMAGE_VAULTS: &str = "image-vaults";

/// `<place>/<name>`. internal.ts:69.
pub fn copy_key(place: &str, name: &str) -> String {
    format!("{place}/{name}")
}

/// A stored copy key read back: split at the first "/"; a key with none names no place. internal.ts:72-75.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CopyKeyParts {
    pub place: Option<String>,
    pub name: String,
}

/// internal.ts:72-75.
pub fn copy_key_parts(key: &str) -> CopyKeyParts {
    match key.find('/') {
        None => CopyKeyParts { place: None, name: key.to_string() },
        Some(at) => CopyKeyParts { place: Some(key[..at].to_string()), name: key[at + 1..].to_string() },
    }
}

/// `<name>@v<version>`; `vault_key` is the same rule. internal.ts:76-77.
pub fn recipe_key(name: &str, version: u32) -> String {
    format!("{name}@v{version}")
}

/// internal.ts:77.
pub fn vault_key(name: &str, version: u32) -> String {
    recipe_key(name, version)
}

/// internal.ts:78.
pub const TRANSCRIPTS: &str = "transcripts";
/// internal.ts:81.
pub const TRANSCRIPT_HEADS: &str = "transcript-heads";
/// internal.ts:83.
pub const TRANSCRIPT_INDEX: &str = "transcript-index";
/// internal.ts:86.
pub const ATTACHMENTS: &str = "attachments";
/// internal.ts:89.
pub const ATTACHMENT_KEYS: &str = "attachment-keys";
/// internal.ts:91.
pub const SESSIONS: &str = "sessions";
/// internal.ts:94.
pub const NOTIFY_OWED: &str = "notify-owed";
/// internal.ts:99.
pub const OWED_RETRY_MS: u64 = 30_000;
/// internal.ts:100.
pub const OWED_RETRY_MAX_MS: u64 = 10 * 60_000;
/// internal.ts:101.
pub const OWED_FOR_MS: u64 = 60 * 60_000;
/// internal.ts:105.
pub const HELD_STARTS: &str = "held-starts";

/// A start a computer's threads at once holds back. internal.ts:106-117.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HeldStartRecord {
    pub workspace_id: String,
    pub thread_id: String,
    pub turn_id: String,
    pub session_id: String,
    pub harness: String,
    pub place: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prompt: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub notify: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub notify_by: Option<serde_json::Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub notify_road: Option<serde_json::Value>,
}

/// internal.ts:121.
pub const SUB_STARTS: &str = "sub-starts";

/// A sub-thread's first start as it was asked. internal.ts:128.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SubStartOpts {
    pub prompt: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub harness: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub effort: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub permission_mode: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub access: Option<AccessChoice>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub context_window: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fast: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub started_by: Option<SessionOrigin>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub request_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub attempt: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cwd: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub attachments: Option<Vec<Attachment>>,
}

/// internal.ts:129.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SubTree {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent_thread_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub root_thread_id: Option<String>,
}

/// internal.ts:122-133.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SubStartRecord {
    pub workspace_id: String,
    pub thread_id: String,
    pub turn_id: String,
    /// ISO.
    pub at: String,
    pub opts: SubStartOpts,
    pub tree: SubTree,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub notify: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub by: Option<ThreadScope>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub road: Option<WorkspaceOrigin>,
}

/// internal.ts:137.
pub const WORKSPACE_NAMES: &str = "workspace-names";
/// internal.ts:140.
pub const DROPPED: &str = "dropped-machines";
/// internal.ts:144.
pub const DROPPED_WATCH_MS: u64 = 7 * 24 * 60 * 60_000;
/// internal.ts:147.
pub const UNRECORDED: &str = "unrecorded-machines";
/// internal.ts:148.
pub const UNRECORDED_DELETE_MS: u64 = 24 * 3_600_000;
/// internal.ts:150.
pub const PREFERENCES: &str = "preferences";
/// internal.ts:151.
pub const PREFERENCES_ID: &str = "default";
/// internal.ts:154.
pub const READS: &str = "reads";
/// internal.ts:155.
pub const READS_ID: &str = "since";
/// internal.ts:158.
pub const AGENT_LISTS: &str = "agent-lists";

/// internal.ts:161.
pub const PAUSED_REASON: &str = "machine paused while the agent was working";
/// internal.ts:162.
pub const DELETED_REASON: &str = "machine deleted while the agent was working";
/// internal.ts:163.
pub const RESTARTED_REASON: &str = "host restarted while the agent was working";
/// internal.ts:164.
pub const GONE_REASON: &str = "machine gone at the provider while the agent was working";

// --- log lines -----------------------------------------------------------------------------------------------------

/// The record's name, or `thread <threadWord(id)>` where a thread named the verb. internal.ts:167.
pub fn label_of(record_name: &str, thread_id: Option<&str>) -> String {
    match thread_id {
        None => record_name.to_string(),
        Some(thread) => format!("thread {}", thread_word(thread)),
    }
}
/// internal.ts:169.
pub fn gone_log_line(workspace_id: &str, words: &str) -> String {
    format!("workspace {workspace_id} is gone: {words}")
}
/// internal.ts:171-172.
pub fn swept_runs_log_line(workspace_id: &str, runs: &[String]) -> String {
    let count = if runs.len() == 1 { "1 harness run".to_string() } else { format!("{} harness runs", runs.len()) };
    format!("ended {count} on {workspace_id} that no thread here holds: {}", runs.join(", "))
}
/// internal.ts:177-178.
pub fn server_values_unread_line(agent: &str, reason: &str) -> String {
    format!("{agent}'s MCP servers on that computer could not be read for the values its launch hands them ({reason}), so the turn did not start; try again once it answers")
}
/// internal.ts:181-182.
pub fn server_values_left_line(agent: &str, line: &str) -> String {
    let cap = fmt_bytes(CONFIG_READ_MAX_BYTES as f64);
    format!("{agent}'s MCP servers on that computer were not read for the values its launch hands them: {line}, so the turn did not start; make it a plain file under {cap}, or a link to one inside its folder, and send again")
}
/// internal.ts:185-186.
pub fn server_values_cut_line(agent: &str, files: &[String]) -> String {
    let cap = fmt_bytes(EXEC_OUTPUT_MAX as f64);
    format!("{agent}'s MCP servers on that computer could not be read whole for the values its launch hands them: {} together, once encoded, pass the {cap} one read there carries, so the turn did not start; make them smaller and send again", files.join(" and "))
}
/// The session id cut to its first 8 UTF-16 units. internal.ts:187-188.
pub fn no_title_log_line(session_id: &str, workspace_id: &str, words: &str) -> String {
    format!("no title for session {} on {workspace_id}: {words}", utf16::cut(session_id, 8))
}
/// internal.ts:191-192.
pub fn no_made_title_log_line(thread_id: &str, workspace_id: &str, words: &str) -> String {
    format!("thread {} on {workspace_id} keeps its opening words: {words}", utf16::cut(thread_id, 8))
}
/// internal.ts:194-195.
pub fn no_checkpoint_log_line(thread_id: &str, workspace_id: &str, words: &str) -> String {
    format!("no checkpoint of the files at thread {}'s turn end on workspace {workspace_id}: {words}", utf16::cut(thread_id, 8))
}
/// internal.ts:198-199.
pub fn no_name_write_log_line(session_id: &str, workspace_id: &str, words: &str) -> String {
    format!("the harness did not take the name for session {} on {workspace_id}: {words}", utf16::cut(session_id, 8))
}
/// The session id whole, the thread id cut to 8. internal.ts:202-203.
pub fn lost_session_log_line(session_id: &str, thread_id: &str, workspace_id: &str, words: &str) -> String {
    format!(
        "session {session_id} of thread {} was gone from {workspace_id}, so its turn started a new one: {words}",
        utf16::cut(thread_id, 8)
    )
}
/// internal.ts:205-206.
pub fn stop_log_line(thread: &str, task: Option<&str>, outcome: &str, left: Option<&str>) -> String {
    let subagent = task.map_or(String::new(), |task| format!(" subagent {}", utf16::cut(task, 8)));
    let left = left.map_or(String::new(), |left| format!(" ({left})"));
    format!("stop on thread {}{subagent}: {outcome}{left}", utf16::cut(thread, 8))
}
/// `cut by a host restart after <fmtDuration(ms, "clock")>`. internal.ts:208.
pub fn restart_cut_line(elapsed_ms: f64) -> String {
    format!("cut by a host restart after {}", fmt_duration(elapsed_ms, Some(DurationStyle::Clock)))
}

/// internal.ts:210.
pub const DAEMON_TOKEN_MISS_TTL_MS: u64 = 60_000;
/// Events kept per thread. internal.ts:212.
pub const TRANSCRIPT_CAP: usize = 5000;
/// internal.ts:215.
pub const TRANSCRIPT_BYTES: usize = 4 * 1024 * 1024;
/// internal.ts:219.
pub const TOOL_RESULT_KEPT: usize = 16 * 1024;
/// internal.ts:222.
pub const TRANSCRIPTS_HELD: usize = 4;
/// internal.ts:225.
pub const PENDING_FLUSH_BYTES: usize = 1024 * 1024;
/// setTimeout's longest wait. internal.ts:227.
pub const MAX_TIMER_MS: u64 = (1 << 31) - 1;
/// internal.ts:233.
pub const WORDS_HELD: usize = 2 * 1024 * 1024;

pub use super::internal_index::*;

// --- records -------------------------------------------------------------------------------------------------------

/// internal.ts:559-563.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NamedWorkspace {
    pub workspace_id: String,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project: Option<String>,
}

/// internal.ts:565-568.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DroppedMachine {
    pub machine_id: String,
    pub at: String,
}

/// internal.ts:570-574.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UnrecordedMachine {
    pub machine_id: String,
    pub place: String,
    pub seen_at: String,
}

/// internal.ts:576-579.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TranscriptRecord {
    pub workspace_id: String,
    pub events: Vec<SessionEvent>,
}

/// internal.ts:591.
#[derive(Clone, Debug, PartialEq)]
pub struct ToldAs {
    pub status: TurnStatus,
    pub body: Option<String>,
    pub said: Option<String>,
    pub promised: bool,
}

/// What a live turn knows beyond its row. internal.ts:583-601.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct TurnLive {
    pub reply: Option<TurnStatus>,
    pub told: Option<u64>,
    pub told_as: Option<ToldAs>,
    pub steered: Option<IndexMap<String, Steered>>,
    pub waited: Option<JsNumber>,
    pub waiting_since: Option<JsNumber>,
    /// What failed the turn's stream, as its transport threw it. In memory alone.
    pub failure: Option<HostError>,
}

/// A message steered into a turn. internal.ts:605-615.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Steered {
    pub prompt: String,
    pub started_by: SessionOrigin,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub by: Option<ThreadScope>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub road: Option<WorkspaceOrigin>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub request_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub attachments: Option<Vec<AttachmentRecord>>,
}

/// The steered messages a row kept, read back entry by entry; None where `raw` is not a plain object. internal.ts:619-632.
pub fn read_steered(raw: &serde_json::Value) -> Option<IndexMap<String, Steered>> {
    let entries = raw.as_object()?;
    let mut steered = IndexMap::new();
    for (id, entry) in entries {
        let Some(e) = entry.as_object() else { continue };
        let started_by = e.get("startedBy").and_then(|v| parse::<SessionOrigin>(v).ok());
        let by = read_scope(e.get("by"));
        let road = read_road(e.get("road"));
        let prompt = e.get("prompt").and_then(Value::as_str);
        let (Some(prompt), Some(started_by)) = (prompt, started_by) else { continue };
        if (e.contains_key("by") && by.is_none()) || (e.contains_key("road") && road.is_none()) {
            continue;
        }
        let attachments = e.get("attachments").and_then(|v| parse::<Vec<AttachmentRecord>>(v).ok()).filter(|a| !a.is_empty());
        let request_id = e.get("requestId").and_then(Value::as_str).map(str::to_string);
        steered.insert(id.clone(), Steered { prompt: prompt.to_string(), by, road, started_by, request_id, attachments });
    }
    Some(steered)
}

/// The message a turn's agent was handed, kept beside its run while it runs. internal.ts:637-652.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TurnAsked {
    pub prompt: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub effort: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub typed: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub request_id: Option<String>,
    /// `true` where present.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub owed: Option<bool>,
    /// `true` where present.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub awaits_prompt: Option<bool>,
}

/// internal.ts:655.
pub const KEPT_CLOSE_WAIT_MS: u64 = 2_000;

/// internal.ts:668.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WarmKeep {
    pub harness: String,
    pub claimed: bool,
}

/// A thread's agent process kept up between its turns. internal.ts:661-677.
pub struct KeptProcess {
    pub workspace_id: String,
    pub agent: Box<dyn KeptAgent>,
    pub pid: Option<u32>,
    pub launch: KeptLaunch,
    pub session: Option<String>,
    pub warm: Option<WarmKeep>,
    pub turn_token: String,
    pub scope_device_id: Option<String>,
    pub waiting: Rc<std::cell::Cell<bool>>,
    pub file: Option<SessionFileStamp>,
    pub box_file: Option<futures::future::Shared<futures::future::LocalBoxFuture<'static, Option<i64>>>>,
    pub used_at: JsNumber,
    pub cancel: Box<dyn FnOnce()>,
}

/// A launch as a kept process is matched against. internal.ts:681-684.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct KeptLaunch {
    pub fixed: String,
    pub picks: IndexMap<String, serde_json::Value>,
}

/// The same fixed part, and every pick the send names equal to the kept one's (a pick the kept launch lacks reads
/// as absent, which equals only an absent pick). internal.ts:688-690.
pub fn launches_as(kept: &KeptLaunch, send: &KeptLaunch) -> bool {
    kept.fixed == send.fixed && send.picks.iter().all(|(pick, value)| kept.picks.get(pick) == Some(value))
}

/// How a kept agent's session file stood when its turn ended. internal.ts:694-697.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SessionFileStamp {
    pub path: Option<String>,
    /// -1 where none was found or the stat failed.
    pub size: i64,
}

/// The one file under `folder` whose name ends in `name`, `depth` folders down, folders tried in descending name
/// order (`localeCompare`), the first found winning; None where a folder does not read. internal.ts:700-716.
pub fn find_session_file(folder: &str, name: &str, depth: u32) -> Option<String> {
    let mut entries: Vec<fs::DirEntry> = fs::read_dir(folder).ok()?.filter_map(Result::ok).collect();
    entries.sort_by_key(|e| e.file_name());
    if depth == 0 {
        let file = entries.iter().find(|e| e.file_type().is_ok_and(|t| t.is_file()) && e.file_name().to_string_lossy().ends_with(name))?;
        return Some(node_path::join(&[folder, &file.file_name().to_string_lossy()]));
    }
    let mut dirs: Vec<&fs::DirEntry> = entries.iter().filter(|e| e.file_type().is_ok_and(|t| t.is_dir())).collect();
    dirs.sort_by(|a, b| collate(&b.file_name().to_string_lossy(), &a.file_name().to_string_lossy()));
    for dir in dirs {
        let child = node_path::join(&[folder, &dir.file_name().to_string_lossy()]);
        if let Some(found) = find_session_file(&child, name, depth - 1) {
            return Some(found);
        }
    }
    None
}

/// The file's size, -1 where it does not stat. internal.ts:718-724.
pub fn size_of(path: &str) -> i64 {
    fs::metadata(path).map_or(-1, |m| m.len() as i64)
}

/// None where the agent names no file; else the path found (or `known`) and its size, or `{size: -1}` with no path.
/// internal.ts:726-730.
pub fn stamp_session_file(at: Option<&SessionFile>, known: Option<&str>) -> Option<SessionFileStamp> {
    let at = at?;
    let path = match known {
        Some(path) => Some(path.to_string()),
        None => find_session_file(&at.folder, &at.name, at.depth),
    };
    Some(match path {
        None => SessionFileStamp { path: None, size: -1 },
        Some(path) => {
            let size = size_of(&path);
            SessionFileStamp { path: Some(path), size }
        }
    })
}

/// Whether the session file stands as the stamp found it. internal.ts:734-738.
pub fn same_session_file(at: Option<&SessionFile>, stamp: Option<&SessionFileStamp>) -> bool {
    let (Some(at), Some(stamp)) = (at, stamp) else { return true };
    stamp_session_file(Some(at), stamp.path.as_deref()).is_some_and(|now| now.path == stamp.path && now.size == stamp.size)
}

/// A turn's message read back off the session index. internal.ts:741-745.
pub fn read_asked(raw: &serde_json::Value) -> Option<TurnAsked> {
    let e = raw.as_object()?;
    let text = |key: &str| e.get(key).and_then(Value::as_str).map(str::to_string);
    let flag = |key: &str| (e.get(key) == Some(&Value::Bool(true))).then_some(true);
    Some(TurnAsked {
        prompt: text("prompt")?,
        effort: text("effort"),
        typed: text("typed"),
        request_id: text("requestId"),
        owed: flag("owed"),
        awaits_prompt: flag("awaitsPrompt"),
    })
}

/// A thread scope read back: the schema's, else None. internal.ts:749-753.
pub fn read_scope(raw: Option<&serde_json::Value>) -> Option<ThreadScope> {
    parse::<ThreadScope>(raw?).ok()
}

/// A road read back: the schema's, else None. internal.ts:758-762.
pub fn read_road(raw: Option<&serde_json::Value>) -> Option<WorkspaceOrigin> {
    parse::<WorkspaceOrigin>(raw?).ok()
}

/// `"send" | "list"`. internal.ts:766.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TreeTalk {
    Send,
    List,
}

/// internal.ts:802.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LimitResume {
    pub at: JsNumber,
    pub turn_id: String,
}

/// internal.ts:805.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Rewound {
    pub before: String,
    pub at: JsNumber,
}

/// A thread's own record. internal.ts:772-818. The runtime's map holds it as `ThreadRecord & { workspaceId }`
/// (context.ts:213), `workspaceId` first as every literal that makes one writes it; the index record that stores a
/// thread under its workspace holds none, and an empty one is left unwritten.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ThreadRecord {
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub workspace_id: String,
    pub harness: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub permission_mode: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent_thread_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub root_thread_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub read_at: Option<JsNumber>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub settled_at: Option<JsNumber>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub settle_named_at: Option<JsNumber>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pinned_at: Option<JsNumber>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub order: Option<JsNumber>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub folded_at: Option<JsNumber>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub replaces: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub snoozed_until: Option<JsNumber>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub section: Option<ThreadPlacement>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cut_owed: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub forked_from: Option<ForkedFrom>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit_resume: Option<LimitResume>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rewound: Option<Rewound>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub files_in: Option<Vec<String>>,
    /// Absent reads as worked.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub worked: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ended: Option<TurnStatus>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub clouds: Option<Vec<String>>,
}

/// The stamps a mark moves on a thread's record: every key but harness and permissionMode, one set to `Some(None)`
/// taken off. internal.ts:821.
pub type ThreadStamps = serde_json::Map<String, serde_json::Value>;

/// One running or finished row as the session index keeps it. internal.ts:834.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IndexedSession {
    #[serde(flatten)]
    pub view: SessionView,
    pub turn_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub notify: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub notify_by: Option<ThreadScope>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub notify_road: Option<WorkspaceOrigin>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reply: Option<TurnStatus>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub told: Option<u64>,
    /// An older host's; read and dropped.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub told_last: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub steered: Option<IndexMap<String, Steered>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub waited: Option<JsNumber>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub waiting_since: Option<JsNumber>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub run: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub from: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub asked: Option<TurnAsked>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub turn_token: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scope_device_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub snapshot: Option<String>,
}

/// internal.ts:823-837. Fields in the order the TS writes the record (transcripts.ts persistSessions:
/// `{ workspaceId, sessions, threads, viewed? }`), not the interface's.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionIndexRecord {
    pub workspace_id: String,
    pub sessions: Vec<IndexedSession>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub threads: Option<IndexMap<String, ThreadRecord>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub viewed: Option<IndexMap<String, String>>,
}

/// internal.ts:841.
pub const BUILDERS: &str = "builders";
/// internal.ts:843.
pub const OWNER: &str = "owner";
/// internal.ts:845.
pub const CREATES: &str = "creates";
/// internal.ts:847.
pub const KEY_PURPOSE_MAX: usize = 255 - 17;

/// The create attempt in flight for a record. internal.ts:848-861.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PendingCreate {
    pub key: String,
    pub created_at: String,
    pub body: String,
    pub host: String,
    pub pid: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub request: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub image: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub place: Option<String>,
}

/// internal.ts:864.
pub const LOST_CREATE_MS: u64 = 2 * 60_000;

/// The sha256 hex of the spec's JSON without idempotencyKey and without the createdAt label, `labels` moved last
/// and always present (`{}` where the spec had none). internal.ts:867-873.
pub fn fingerprint(spec: &MachineSpec) -> String {
    let Ok(Value::Object(mut body)) = serde_json::to_value(spec) else { return String::new() };
    body.shift_remove("idempotencyKey");
    let mut labels = match body.shift_remove("labels") {
        Some(Value::Object(labels)) => labels,
        _ => serde_json::Map::new(),
    };
    labels.shift_remove(CREATED_AT_LABEL);
    body.insert("labels".into(), Value::Object(labels));
    hex::encode(Sha256::digest(serde_json::to_string(&Value::Object(body)).unwrap_or_default()))
}

/// internal.ts:875.
pub const HELD_TTL_MS: u64 = 15 * 60_000;
/// internal.ts:877.
pub const HEARTBEAT_MS: u64 = 5 * 60_000;

/// Whether an error is the provider's concurrency refusal: an answered refusal of kind concurrency. internal.ts:879.
pub fn is_cap_refusal(e: &EngineError) -> bool {
    matches!(e, EngineError::Provider(w) if w.kind == ErrorKind::Concurrency)
}

/// `kill(pid, 0)` succeeds, or fails with EPERM. internal.ts:881-888.
pub fn pid_alive(pid: i32) -> bool {
    // SAFETY: signal 0 only checks that the process exists.
    unsafe { libc::kill(pid, 0) == 0 || std::io::Error::last_os_error().raw_os_error() == Some(libc::EPERM) }
}

/// internal.ts:904.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct HeldBy {
    pub host: String,
    pub pid: u32,
    pub heartbeat: String,
}

/// internal.ts:917.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Sealed {
    pub at: String,
    pub version: u32,
}

/// An image builder's record. internal.ts:890-921.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BuilderRecord {
    pub id: String,
    pub name: String,
    pub kind: MachineKind,
    pub base_template: String,
    pub setup_sha: String,
    pub created_at: String,
    pub size: WorkspaceSize,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stream_url: Option<String>,
    pub first_life: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub held_by: Option<HeldBy>,
    /// `true` where present.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub building: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub import: Option<ImportLedger>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub base: Option<Vec<GoldenBaseTool>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent_snapshot_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub retired: Option<Vec<GoldenRetired>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sealed: Option<Sealed>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub place: Option<String>,
}

/// own, reusable, stale, foreign, held. internal.ts:934.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BuilderLife {
    Own,
    Reusable,
    Stale,
    Foreign,
    Held,
}

/// internal.ts:923-938.
pub struct LiveBuilder {
    pub record: BuilderRecord,
    pub builder: Builder,
    pub sealable: bool,
    pub life: BuilderLife,
    pub reach: Option<PreviewReach>,
    pub recipe: Option<GoldenRecipe>,
}

/// What prepare rejects with once its signal aborted. internal.ts:943-964.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PrepareStoppedError {
    pub builder_id: Option<String>,
    pub kept: bool,
    pub left: Option<String>,
}

/// How a stopped prepare left its builder.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PrepareOutcome {
    Kept,
    Left(String),
}

impl PrepareStoppedError {
    /// internal.ts:947-963.
    pub fn new(builder_id: Option<String>, outcome: Option<PrepareOutcome>) -> PrepareStoppedError {
        let kept = outcome == Some(PrepareOutcome::Kept);
        let left = match outcome {
            Some(PrepareOutcome::Left(why)) => Some(why),
            _ => None,
        };
        PrepareStoppedError { builder_id, kept, left }
    }

    /// The four messages of internal.ts:950-958.
    pub fn message(&self) -> String {
        match (&self.builder_id, &self.left) {
            (None, _) => "prepare stopped before a machine existed".to_string(),
            (Some(id), _) if self.kept => format!("prepare stopped; builder {id} left running with a seal still in it"),
            (Some(id), None) => format!("prepare stopped; builder {id} killed"),
            (Some(id), Some(why)) => format!("prepare stopped; builder {id} did not stop: {why}"),
        }
    }
}

/// The stand-in a dead machine is: every call fails as a missing machine, kill succeeds, state answers gone.
struct DeadMachine {
    id: String,
}

fn gone(id: &str) -> EngineError {
    EngineError::Provider(WspError {
        kind: ErrorKind::Missing,
        status: 404,
        code: None,
        message: format!("machine {id} is gone"),
        request_id: None,
        window: None,
        said: None,
    })
}

#[async_trait(?Send)]
impl Machine for DeadMachine {
    fn id(&self) -> &str {
        &self.id
    }
    fn kind(&self) -> MachineKind {
        MachineKind::Sandbox
    }
    async fn exec(&self, _cmd: &str, _opts: ExecOptions) -> Result<ExecResult, EngineError> {
        Err(gone(&self.id))
    }
    async fn run(&self, _script: &str, _opts: RunOptions) -> Result<ExecResult, EngineError> {
        Err(gone(&self.id))
    }
    async fn snapshot(&self, _name: &str, _life: MachineLife, _opts: SnapshotOptions) -> Result<String, EngineError> {
        Err(gone(&self.id))
    }
    async fn pause(&self, _wait_ms: Option<u64>) -> Result<(), EngineError> {
        Err(gone(&self.id))
    }
    async fn resume(&self, _signal: Option<&Cancel>) -> Result<(), EngineError> {
        Err(gone(&self.id))
    }
    async fn kill(&self) -> Result<(), EngineError> {
        Ok(())
    }
    async fn state(&self) -> Result<MachineState, EngineError> {
        Ok(MachineState::Gone)
    }
    async fn download_url(&self, _path: &str) -> Result<String, EngineError> {
        Err(gone(&self.id))
    }
    async fn upload_url(&self, _path: &str) -> Result<String, EngineError> {
        Err(gone(&self.id))
    }
}

/// The stand-in an absent machine is: every call, kill and state included, fails with the caller's away error.
struct AbsentMachine {
    id: String,
    kind: MachineKind,
    away: Arc<dyn Fn() -> EngineError>,
}

thread_local! {
    /// The addresses of the live absent machines, since a `dyn Machine` cannot say what it is.
    static ABSENT: RefCell<BTreeSet<usize>> = const { RefCell::new(BTreeSet::new()) };
}

impl Drop for AbsentMachine {
    fn drop(&mut self) {
        ABSENT.with(|set| set.borrow_mut().remove(&(self as *const Self as usize)));
    }
}

#[async_trait(?Send)]
impl Machine for AbsentMachine {
    fn id(&self) -> &str {
        &self.id
    }
    fn kind(&self) -> MachineKind {
        self.kind
    }
    async fn exec(&self, _cmd: &str, _opts: ExecOptions) -> Result<ExecResult, EngineError> {
        Err((self.away)())
    }
    async fn run(&self, _script: &str, _opts: RunOptions) -> Result<ExecResult, EngineError> {
        Err((self.away)())
    }
    async fn snapshot(&self, _name: &str, _life: MachineLife, _opts: SnapshotOptions) -> Result<String, EngineError> {
        Err((self.away)())
    }
    async fn pause(&self, _wait_ms: Option<u64>) -> Result<(), EngineError> {
        Err((self.away)())
    }
    async fn resume(&self, _signal: Option<&Cancel>) -> Result<(), EngineError> {
        Err((self.away)())
    }
    async fn kill(&self) -> Result<(), EngineError> {
        Err((self.away)())
    }
    async fn state(&self) -> Result<MachineState, EngineError> {
        Err((self.away)())
    }
    async fn download_url(&self, _path: &str) -> Result<String, EngineError> {
        Err((self.away)())
    }
    async fn upload_url(&self, _path: &str) -> Result<String, EngineError> {
        Err((self.away)())
    }
}

/// Stand-in for a machine that vanished while we were away: every call but kill and state fails with
/// `EngineError::Provider` of kind missing, status 404, message `machine <id> is gone`; kill succeeds; state answers
/// gone; kind sandbox, no stream url. internal.ts:968-998.
pub fn dead_machine(id: &str) -> Arc<dyn Machine> {
    Arc::new(DeadMachine { id: id.to_string() })
}

/// What a released record stands on. internal.ts:1002.
pub const RELEASED_LINE: &str = "this sub-thread's machine was deleted at its cloud, so there is none to ask";

/// `new NoProviderBackend(RELEASED_LINE)`. internal.ts:1002.
pub fn released_backend() -> Arc<dyn MachineBackend> {
    Arc::new(NoProviderBackend::new(Some(RELEASED_LINE.to_string())))
}

/// Whether this handle is the stand-in `absent_machine` makes. internal.ts:1009.
pub fn is_absent_machine(m: &dyn Machine) -> bool {
    let addr = m as *const dyn Machine as *const () as usize;
    ABSENT.with(|set| set.borrow().contains(&addr))
}

/// The machine a record nothing can be asked about is held by: every call, kill and state included, fails with
/// `away()`. internal.ts:1015-1031.
pub fn absent_machine(id: &str, kind: MachineKind, away: Arc<dyn Fn() -> EngineError>) -> Arc<dyn Machine> {
    let machine = Arc::new(AbsentMachine { id: id.to_string(), kind, away });
    let addr = Arc::as_ptr(&machine) as usize;
    ABSENT.with(|set| set.borrow_mut().insert(addr));
    machine
}

/// Reads at most `cap` bytes of a body and drops the rest; a body that fails mid-read answers what came. The bytes
/// are cut at exactly `cap` and decoded as UTF-8 with replacement. internal.ts:1034-1051.
pub async fn read_body_up_to(body: &mut (dyn futures::Stream<Item = Result<Vec<u8>, HostError>> + Unpin), cap: usize) -> String {
    let mut chunks: Vec<Vec<u8>> = Vec::new();
    let mut size = 0;
    while size < cap {
        match body.next().await {
            Some(Ok(chunk)) => {
                size += chunk.len();
                chunks.push(chunk);
            }
            _ => break,
        }
    }
    let bytes = chunks.concat();
    String::from_utf8_lossy(&bytes[..size.min(cap)]).into_owned()
}

/// internal.ts:1055.
pub const OPTIONAL_BACKEND_CALLS: [&str; 10] = [
    "checkKey",
    "limits",
    "listSnapshots",
    "promoteSnapshot",
    "getTemplate",
    "listTemplates",
    "deleteTemplate",
    "createRequest",
    "replayCreate",
    "namedAfter",
];

/// What a daemon refused a frame with, the code beside the sentence. internal.ts:1070-1078.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
#[error("{message}")]
pub struct DaemonRefusal {
    pub code: Option<String>,
    pub message: String,
}

/// internal.ts:1082.
pub fn is_no_host_cli(e: &DaemonRefusal) -> bool {
    e.code.as_deref() == Some("no-host-cli")
}
/// internal.ts:1083.
pub fn is_no_git_credential(e: &DaemonRefusal) -> bool {
    e.code.as_deref() == Some("no-git-credential")
}
/// internal.ts:1085.
pub fn is_on_default_branch(e: &DaemonRefusal) -> bool {
    e.code.as_deref() == Some("on-default-branch")
}

/// `refusal(defaultBranchRefusal(workspace, branch), defaultBranchFix(road, workspace), "usage")`. internal.ts:1089-1090.
pub fn default_branch_refused(road: DefaultBranchRoad, workspace: &str, branch: &str) -> HostError {
    refusal(&default_branch_refusal(workspace, branch), &default_branch_fix(road, workspace), Some(Kind::Usage))
}

/// `"environment" | "launch" | "file"`. internal.ts:1179.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ServerValuesRoad {
    Environment,
    Launch,
    File,
}

/// What `KindModule::admit_folder` reads of a record.
#[derive(Clone, Debug, PartialEq)]
pub struct FolderAdmit {
    pub kind: WorkspaceKind,
    pub place: Option<String>,
    pub name: String,
}

/// internal.ts:1146.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct EndThread {
    pub remove: bool,
    pub away: bool,
}

/// internal.ts:1153.
#[derive(Clone, Debug, PartialEq)]
pub struct PortRoute {
    pub url: String,
    pub expires_at: JsNumber,
}

/// The one place a workspace's kind means anything. Optional members of the TS answer None here. internal.ts:1097-1214.
pub trait KindModule {
    /// Refuses where the record's place forks nowhere (kinds.ts:327): the TS throws there.
    fn backend(&self, record: &WorkspaceRecord) -> Result<Arc<dyn MachineBackend>, HostError>;
    fn exec_stream(
        &self,
        entry: &LiveWorkspace,
        opts: Option<MachineExecOptions>,
        waiting: Option<TurnWaiting>,
    ) -> Arc<dyn ExecStreamFactory>;
    fn admit_folder(&self, record: FolderAdmit) -> Option<Local<'static, ()>>;
    fn folder(&self, record: &WorkspaceRecord) -> Option<String>;
    fn home(&self, entry: &LiveWorkspace, agent_id: &str) -> String;
    fn home_dir(&self, record: &WorkspaceRecord) -> Option<String>;
    fn env(&self, entry: &LiveWorkspace, agent_id: Option<&str>, home_of: &dyn Fn(&str) -> String) -> IndexMap<String, String>;
    fn login_stands(&self, entry: &LiveWorkspace, agent_id: &str) -> bool;
    fn import(&self, entry: &LiveWorkspace, o: ProjectImportOptions, report: ImportReport) -> Local<'static, ImportLanded>;
    fn roots(&self, entry: &LiveWorkspace, dests: &[String]) -> Local<'static, ()>;
    fn land_project(&self, entry: &LiveWorkspace, project: ProjectView, report: StageReport) -> Local<'static, ()>;
    fn memory_key(&self, entry: &LiveWorkspace, agent_id: &str) -> Option<String>;
    fn end_thread(&self, entry: &LiveWorkspace, thread_id: &str, o: EndThread) -> Option<Local<'static, Option<String>>>;
    /// The end owed, with `paid` settling once it ran; present beside end_thread.
    fn end_owed(&self, entry: &LiveWorkspace, thread_id: &str) -> Option<Local<'static, Option<Local<'static, ()>>>>;
    fn port_reach(&self, entry: &LiveWorkspace, port: u16, standing: bool) -> Option<Local<'static, PortRoute>>;
    fn relayed(&self, machine_id: Option<&str>) -> bool;
    fn wsp_mcp(&self, entry: &LiveWorkspace) -> Option<McpServerSpec>;
    /// Some(None) is a reach with no url.
    fn turn_reach(&self, entry: &LiveWorkspace) -> Option<Option<String>>;
    fn turn_road(&self) -> ScopedRoad;
    fn keeps_agents(&self) -> bool;
    fn server_values(&self) -> ServerValuesRoad;
    fn has_daemon(&self, entry: &LiveWorkspace) -> bool;
    fn shared_daemon(&self) -> bool;
    fn daemon_road(&self, entry: &LiveWorkspace) -> Local<'static, wsp_wire::views::workspace::DaemonReachView>;
    fn scratch(&self, entry: &LiveWorkspace) -> String;
    fn dropped(&self, entry: &LiveWorkspace) -> Local<'static, ()>;
    fn daemon_version(&self, entry: &LiveWorkspace) -> Local<'static, Option<u32>>;
    fn restart_daemon(&self, entry: &LiveWorkspace) -> Option<Local<'static, ()>>;
    fn deploy_daemon(&self, entry: &LiveWorkspace) -> Option<Local<'static, Option<String>>>;
}

/// internal.ts:1215.
pub type ImportReport = Rc<dyn Fn(ProjectImportStage, &str, Option<(u64, u64)>)>;

/// internal.ts:1216-1219.
#[derive(Clone, Debug, PartialEq)]
pub struct ImportLanded {
    pub result: ProjectImportResult,
    pub done: String,
}

/// What the request rules need of a workspace. internal.ts:1223.
#[derive(Clone, Debug, PartialEq)]
pub struct WorkspaceLike {
    pub id: Option<String>,
    pub kind: WorkspaceKind,
    pub name: String,
    pub machine_id: Option<String>,
    pub root_thread_id: Option<String>,
    pub project: Option<String>,
    pub worktree: Option<WorktreeFolder>,
}

/// One build stage as its frame carries it. internal.ts:1226-1232.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct StageFrame {
    pub name: String,
    pub stage: GoldenStage,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub step: Option<GoldenStep>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub left: Option<Vec<String>>,
}

/// internal.ts:1235.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KeptImage {
    pub key: String,
    pub media_type: String,
}

/// One workspace's kept images, by thread. internal.ts:1235.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct KeptImages {
    pub threads: IndexMap<String, Vec<KeptImage>>,
}

/// internal.ts:1239-1243.
#[derive(Clone, Debug, PartialEq)]
pub struct ChildStart {
    pub base: Option<String>,
    pub on_leads: bool,
    pub lines: Vec<String>,
}

/// internal.ts:1246.
#[derive(Clone, Debug, PartialEq)]
pub struct MachineMoment {
    pub machine_id: String,
    pub at: JsNumber,
}

/// `Omit<BuilderRecord, "size" | "firstLife"> & { size?, firstLife? }`: a stored builder from before both were kept.
/// internal.ts:1248.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StoredBuilder {
    pub id: String,
    pub name: String,
    pub kind: MachineKind,
    pub base_template: String,
    pub setup_sha: String,
    pub created_at: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub size: Option<WorkspaceSize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stream_url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub first_life: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub held_by: Option<HeldBy>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub building: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub import: Option<ImportLedger>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub base: Option<Vec<GoldenBaseTool>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent_snapshot_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub retired: Option<Vec<GoldenRetired>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sealed: Option<Sealed>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub place: Option<String>,
}

/// internal.ts:1250.
pub struct LiveSession {
    pub view: SessionView,
    pub turn_id: String,
    pub handle: Rc<dyn SessionHandle>,
    pub turn_live: Option<TurnLive>,
}

/// What re-opening a turn the store left running came to. internal.ts:1257.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Reopened {
    Attached,
    Gone,
    Unreached,
    Cannot,
}

/// A row's end callback: reason, stopped, unreached. internal.ts:1259.
pub type EndRow = Rc<dyn Fn(&str, Option<bool>, Option<bool>)>;

/// internal.ts:1259.
#[derive(Clone, Debug, PartialEq)]
pub struct CallSeen {
    pub tool_name: String,
    pub input: String,
}

/// One row the runtime holds, a turn of a thread. internal.ts:1259.
pub struct SessionEntry {
    pub view: SessionView,
    pub turn_id: String,
    pub replaces: Option<String>,
    pub notify: Option<Vec<String>>,
    pub notify_by: Option<ThreadScope>,
    pub notify_road: Option<WorkspaceOrigin>,
    pub turn_token: Option<String>,
    pub scope_device_id: Option<String>,
    pub handle: Option<Rc<dyn SessionHandle>>,
    pub end: Option<EndRow>,
    pub turn_live: Option<TurnLive>,
    pub run: Option<String>,
    pub from: Option<u64>,
    pub asked: Option<TurnAsked>,
    pub snapshot: Option<String>,
    pub pid: Option<u32>,
    pub launch: Option<futures::future::Shared<futures::future::LocalBoxFuture<'static, ()>>>,
    pub calls: Option<IndexMap<String, CallSeen>>,
    pub unanswered: bool,
    pub queued: bool,
}
