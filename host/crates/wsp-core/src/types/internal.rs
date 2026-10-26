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

use indexmap::{IndexMap, IndexSet};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
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
use wsp_wire::views::session::{ForkedFrom, SessionOrigin, SessionView, SubagentState, SubagentView, ThreadPlacement};
use wsp_wire::views::session_events::{copied_from_of, SessionEvent, TurnStatus};
use wsp_wire::views::workspace::ProjectView;
use wsp_wire::views::workspace_kind::WorkspaceKind;
use wsp_wire::views::workspace_more::{ThreadScope, WorkspaceOrigin, WorktreeFolder};
use wsp_wire::wire::capabilities::WorkspaceSize;
use wsp_wire::wire::daemon::EXEC_OUTPUT_MAX;
use wsp_wire::wire::machine_link::{ExecResult, MachineSpec, PreviewReach};
use wsp_words::exit::{refusal, HostError, Kind};
use wsp_words::js::{collate, js_collapse_whitespace, js_trim, utf16};
use wsp_words::words::computer::{default_branch_fix, default_branch_refusal, thread_word, DefaultBranchRoad};
use wsp_words::words::turn::{listed_failure, listed_last_line};
use wsp_words::words::units::{fmt_bytes, fmt_duration, DurationStyle};

use super::pending::{BlobMark, Builder, ImportLedger, MachineExecOptions, ScopedRoad, TurnWaiting};
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

// --- the transcript index ------------------------------------------------------------------------------------------

/// One thread's words as search reads them. Every length here (`size`, `room`, a line's) counts UTF-16 code units,
/// as JavaScript's `length` does. Keys serialize in this order; `open` is written while it names a message, and left
/// out where it is none, as `JSON.stringify` drops `undefined`. internal.ts:236-251.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ThreadWords {
    pub lines: Vec<String>,
    /// `<turnId or sessionId>:<messageId or "">` of the reply still being written.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub open: Option<String>,
    pub last: JsNumber,
    /// Absent on an index written before WORDS_HELD; `index_read` fills it.
    #[serde(default)]
    pub size: usize,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub first: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub opened: Option<JsNumber>,
    /// `true` where present.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cold: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub room: Option<usize>,
}

/// The first line a thread keeps when its lines go: its first 200 UTF-16 units. internal.ts:253.
pub const FIRST_KEPT: usize = 200;

/// A line cut to FIRST_KEPT UTF-16 units, an owned copy. internal.ts:256.
pub fn first_of(line: &str) -> String {
    utf16::cut(line, FIRST_KEPT)
}

/// An event's keys read by name, as the TS reads `e.threadId` and the like.
struct Fields(Value);

impl Fields {
    fn of(e: &SessionEvent) -> Fields {
        Fields(serde_json::to_value(e).unwrap_or(Value::Null))
    }
    fn str(&self, key: &str) -> Option<&str> {
        self.0.get(key)?.as_str()
    }
    fn num(&self, key: &str) -> Option<f64> {
        self.0.get(key)?.as_f64()
    }
    fn flag(&self, key: &str) -> Option<bool> {
        self.0.get(key)?.as_bool()
    }
    fn has(&self, key: &str) -> bool {
        self.0.get(key).is_some()
    }
}

fn push_line(words: &mut ThreadWords, line: &str) {
    words.lines.push(line.to_string());
    words.size += utf16::len(line);
    if words.first.is_none() {
        words.first = Some(first_of(line));
    }
}

/// A subagent as the index holds it: what a listing answers, the turn whose end stops it, and its newest start row's
/// `at`. internal.ts:283.
#[derive(Clone, Debug, PartialEq)]
pub struct Child {
    pub view: SubagentView,
    pub turn_id: Option<String>,
    pub start_row: Option<JsNumber>,
    /// Whether `usage` was first set by the row that ended the child, which writes it after the end's own keys; it takes
    /// no part in equality.
    pub usage_late: LateUsage,
}

/// Whether a child's `usage` key came after its end's keys.
#[derive(Clone, Copy, Debug, Default)]
pub struct LateUsage(pub bool);

impl PartialEq for LateUsage {
    fn eq(&self, _: &LateUsage) -> bool {
        true
    }
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ChildWire {
    #[serde(flatten)]
    view: SubagentView,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    turn_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    start_row: Option<JsNumber>,
}

/// The keys of a child as the TS object holds them: the running row's, then the end's, and `usage` after them when the
/// end was the first to carry it.
const CHILD_KEYS: [&str; 14] = [
    "id",
    "title",
    "state",
    "parentToolUseId",
    "depth",
    "model",
    "asked",
    "usage",
    "startedAt",
    "turnId",
    "startRow",
    "endedAt",
    "lastLine",
    "failure",
];

impl Serialize for Child {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        use serde::ser::Error;
        let wire = ChildWire { view: self.view.clone(), turn_id: self.turn_id.clone(), start_row: self.start_row };
        let serde_json::Value::Object(mut held) = serde_json::to_value(&wire).map_err(S::Error::custom)? else {
            return Err(S::Error::custom("a child is an object"));
        };
        let mut out = serde_json::Map::new();
        for key in CHILD_KEYS.iter().filter(|k| **k != "usage" || !self.usage_late.0) {
            if let Some(value) = held.remove(*key) {
                out.insert((*key).to_string(), value);
            }
        }
        if let Some(usage) = held.remove("usage") {
            out.insert("usage".to_string(), usage);
        }
        out.extend(held);
        serde_json::Value::Object(out).serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for Child {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        use serde::de::Error;
        let value = serde_json::Value::deserialize(deserializer)?;
        let at = |key: &str| value.as_object().and_then(|o| o.keys().position(|k| k == key));
        let late = matches!((at("usage"), at("startedAt")), (Some(usage), Some(started)) if usage > started);
        let wire: ChildWire = serde_json::from_value(value).map_err(D::Error::custom)?;
        Ok(Child { view: wire.view, turn_id: wire.turn_id, start_row: wire.start_row, usage_late: LateUsage(late) })
    }
}

/// `"started" | "steered"`. internal.ts:289.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TakenOutcome {
    Started,
    Steered,
}

/// internal.ts:285-290.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Taken {
    pub session_id: String,
    pub thread_id: String,
    pub turn_id: String,
    pub outcome: TakenOutcome,
}

/// internal.ts:293.
pub fn transcript_unread_line(workspace_id: &str, why: &str) -> String {
    format!("the transcript of {workspace_id} does not read ({why})")
}

/// cwd, permissionMode, model, effort, in that order. internal.ts:296.
pub const SESSION_FACTS: [&str; 4] = ["cwd", "permissionMode", "model", "effort"];

/// What a resumed session's turns carry, read off its newest start that named each. Written in the order each key was
/// first set, as the TS object holds them. internal.ts:297.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SessionFacts {
    pub cwd: Option<String>,
    pub permission_mode: Option<String>,
    pub model: Option<String>,
    pub effort: Option<String>,
    /// The keys in the order they were first set; it takes no part in equality.
    pub order: FactOrder,
}

/// The order a session's facts were first set in.
#[derive(Clone, Debug, Default)]
pub struct FactOrder(pub Vec<&'static str>);

impl PartialEq for FactOrder {
    fn eq(&self, _: &FactOrder) -> bool {
        true
    }
}

impl SessionFacts {
    fn set(&mut self, key: &'static str, value: &str) {
        let slot = match key {
            "cwd" => &mut self.cwd,
            "permissionMode" => &mut self.permission_mode,
            "model" => &mut self.model,
            _ => &mut self.effort,
        };
        *slot = Some(value.to_string());
        if !self.order.0.contains(&key) {
            self.order.0.push(key);
        }
    }

    fn value(&self, key: &str) -> Option<&String> {
        match key {
            "cwd" => self.cwd.as_ref(),
            "permissionMode" => self.permission_mode.as_ref(),
            "model" => self.model.as_ref(),
            _ => self.effort.as_ref(),
        }
    }
}

impl Serialize for SessionFacts {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeMap;
        let keys: Vec<&str> = self.order.0.iter().copied().chain(SESSION_FACTS).filter(|k| self.value(k).is_some()).collect();
        let mut seen: Vec<&str> = Vec::new();
        let mut map = serializer.serialize_map(None)?;
        for key in keys {
            if seen.contains(&key) {
                continue;
            }
            seen.push(key);
            map.serialize_entry(key, &self.value(key))?;
        }
        map.end()
    }
}

impl<'de> Deserialize<'de> for SessionFacts {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        use serde::de::Error;
        let map = serde_json::Map::<String, serde_json::Value>::deserialize(deserializer)?;
        let mut facts = SessionFacts::default();
        for (key, value) in &map {
            let Some(known) = SESSION_FACTS.into_iter().find(|k| k == key) else { continue };
            let text = value.as_str().ok_or_else(|| D::Error::custom(format!("{key} is not a string")))?;
            facts.set(known, text);
        }
        Ok(facts)
    }
}

/// What the host keeps of a transcript it does not hold. Every map iterates in insertion order (chapter 03, rule
/// 3.11): a delete is `shift_remove`. internal.ts:261-279.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct TranscriptIndex {
    pub words: IndexMap<String, ThreadWords>,
    pub starts: IndexMap<String, String>,
    pub cut: IndexMap<String, bool>,
    pub facts: IndexMap<String, SessionFacts>,
    pub taken: IndexMap<String, Taken>,
    pub children: IndexMap<String, IndexMap<String, Child>>,
    pub trimmed: IndexSet<String>,
    /// The newest position issued; never moves back.
    pub pos: u64,
}

/// internal.ts:299.
pub fn empty_index() -> TranscriptIndex {
    TranscriptIndex::default()
}

/// One session.subagent row into a thread's children. internal.ts:305-332. The event must be a session.subagent;
/// any other is ignored.
pub fn fold_child(index: &mut TranscriptIndex, e: &SessionEvent) {
    let SessionEvent::SessionSubagentEvent(e) = e else { return };
    let Some(thread) = e.thread_id.as_deref() else { return };
    let child = index.children.get(thread).and_then(|held| held.get(&e.task)).cloned();
    if e.state == SubagentState::Running {
        let view = SubagentView {
            id: e.task.clone(),
            title: e.title.clone().or_else(|| child.as_ref().map(|c| c.view.title.clone())).unwrap_or_else(|| e.task.clone()),
            state: SubagentState::Running,
            parent_tool_use_id: e.parent_tool_use_id.clone().or_else(|| child.as_ref().and_then(|c| c.view.parent_tool_use_id.clone())),
            depth: e.depth.or_else(|| child.as_ref().and_then(|c| c.view.depth)),
            model: e.model.clone().or_else(|| child.as_ref().and_then(|c| c.view.model.clone())),
            asked: e.asked.clone().or_else(|| child.as_ref().and_then(|c| c.view.asked.clone())),
            started_at: child.as_ref().map(|c| c.view.started_at).or(e.at).unwrap_or(JsNumber(0.0)),
            ended_at: None,
            last_line: None,
            failure: None,
            usage: e.usage.clone().or_else(|| child.as_ref().and_then(|c| c.view.usage.clone())),
        };
        let row = Child { view, turn_id: e.turn_id.clone(), start_row: e.at, usage_late: LateUsage(false) };
        index.children.entry(thread.to_string()).or_default().insert(e.task.clone(), row);
    } else if let Some(child) = index.children.get_mut(thread).and_then(|held| held.get_mut(&e.task)) {
        child.view.state = e.state;
        child.view.ended_at = Some(e.at.unwrap_or(child.view.started_at));
        if e.state == SubagentState::Done {
            if let Some(last) = listed_last_line(e.summary.as_deref()) {
                child.view.last_line = Some(last);
            }
        }
        if e.state == SubagentState::Failed && e.summary.is_some() {
            child.view.failure = Some(listed_failure(e.summary.as_deref()));
        }
        if e.usage.is_some() {
            if child.view.usage.is_none() {
                child.usage_late = LateUsage(true);
            }
            child.view.usage = e.usage.clone();
        }
    }
}

/// The row with its turns' words left out: no lastLine, no failure, and each subagent without asked, lastLine and
/// failure. internal.ts:336-339.
pub fn named_only(view: &SessionView) -> SessionView {
    let mut out = view.clone();
    out.last_line = None;
    out.failure = None;
    if let Some(subagents) = out.subagents.as_mut() {
        for child in subagents.iter_mut() {
            child.asked = None;
            child.last_line = None;
            child.failure = None;
        }
    }
    out
}

/// What a turn's end left, onto its row: each line set where given and removed where not. internal.ts:342-348.
pub fn write_lines(view: &mut SessionView, last_line: Option<String>, failure: Option<String>) {
    view.last_line = last_line;
    view.failure = failure;
}

/// A turn's end stops every running child of that turn. internal.ts:351-362.
pub fn end_children(index: &mut TranscriptIndex, thread_id: &str, turn_id: Option<&str>, at: Option<f64>) {
    let Some(held) = index.children.get_mut(thread_id) else { return };
    for child in held.values_mut() {
        if child.view.state != SubagentState::Running || child.turn_id.as_deref() != turn_id {
            continue;
        }
        child.view.state = SubagentState::Stopped;
        child.view.ended_at = Some(at.map_or(child.view.started_at, JsNumber));
        if let Some(usage) = child.view.usage.as_mut() {
            usage.duration_ms = None;
        }
    }
}

/// A row the ring dropped: a child whose start it was leaves the index. internal.ts:365-371.
pub fn forget_child(index: &mut TranscriptIndex, e: &SessionEvent) {
    let SessionEvent::SessionSubagentEvent(e) = e else { return };
    if e.state != SubagentState::Running {
        return;
    }
    let Some(thread) = e.thread_id.as_deref() else { return };
    let Some(held) = index.children.get_mut(thread) else { return };
    if held.get(&e.task).and_then(|c| c.start_row) != e.at {
        return;
    }
    held.shift_remove(&e.task);
    if held.is_empty() {
        index.children.shift_remove(thread);
    }
}

/// One event into an index. internal.ts:376-403.
pub fn fold_event(index: &mut TranscriptIndex, e: &SessionEvent) {
    let f = Fields::of(e);
    if let Some(pos) = f.num("pos") {
        if pos > index.pos as f64 {
            index.pos = pos as u64;
        }
    }
    if copied_from_of(e).is_some() {
        match f.str("type") {
            Some("session.subagent") => fold_child(index, e),
            Some("session.end") => {
                if let Some(thread) = f.str("threadId") {
                    end_children(index, thread, f.str("turnId"), f.num("at"));
                }
            }
            _ => {}
        }
        fold_words(index, e);
        return;
    }
    let ty = f.str("type");
    let (thread, turn) = (f.str("threadId"), f.str("turnId"));
    if let (Some("session.start" | "session.steer"), Some(request), Some(thread), Some(turn)) = (ty, f.str("requestId"), thread, turn) {
        let outcome = if ty == Some("session.start") { TakenOutcome::Started } else { TakenOutcome::Steered };
        let taken = Taken {
            session_id: f.str("sessionId").unwrap_or_default().to_string(),
            thread_id: thread.to_string(),
            turn_id: turn.to_string(),
            outcome,
        };
        index.taken.insert(request.to_string(), taken);
    }
    if ty == Some("session.end") && f.flag("promptless") == Some(true) {
        if let Some(turn) = turn {
            index.taken.retain(|_, t| !(t.turn_id == turn && t.outcome == TakenOutcome::Started));
        }
    }
    match ty {
        Some("session.start") => {
            let session = f.str("sessionId").unwrap_or_default().to_string();
            if let Some(thread) = thread {
                index.starts.insert(thread.to_string(), session.clone());
            }
            let facts = index.facts.entry(session).or_default();
            if let Some(v) = f.str("cwd") {
                facts.set("cwd", v);
            }
            if let Some(v) = f.str("permissionMode") {
                facts.set("permissionMode", v);
            }
            if let Some(v) = f.str("model") {
                facts.set("model", v);
            }
            if let Some(v) = f.str("effort") {
                facts.set("effort", v);
            }
        }
        Some("session.end") if thread.is_some() && f.flag("unstarted") != Some(true) => {
            if let Some(thread) = thread {
                let cut = f.0["exitCode"].is_null() && !f.flag("sawResult").unwrap_or(false);
                index.cut.insert(thread.to_string(), cut);
                end_children(index, thread, turn, f.num("at"));
            }
        }
        Some("session.subagent") => fold_child(index, e),
        _ => {}
    }
    fold_words(index, e);
}

/// A thread's messages and its own agent's replies into the index's words. internal.ts:407-433.
pub fn fold_words(index: &mut TranscriptIndex, e: &SessionEvent) {
    let f = Fields::of(e);
    let Some(thread) = f.str("threadId") else { return };
    let held = index.words.entry(thread.to_string()).or_default();
    held.last = JsNumber(held.last.0.max(f.num("at").unwrap_or(0.0)));
    if held.cold == Some(true) {
        return;
    }
    match f.str("type") {
        Some("session.start") if f.has("prompt") => {
            push_line(held, f.str("prompt").unwrap_or_default());
            held.open = None;
        }
        Some("session.steer") => {
            push_line(held, f.str("prompt").unwrap_or_default());
            held.open = None;
        }
        Some("session.earlier") if matches!(f.str("who"), Some("person" | "agent")) => {
            push_line(held, f.str("text").unwrap_or_default());
            held.open = None;
        }
        Some("session.delta") if f.str("kind") == Some("text") && !f.has("parentToolUseId") => {
            let message =
                format!("{}:{}", f.str("turnId").or(f.str("sessionId")).unwrap_or_default(), f.str("messageId").unwrap_or_default());
            let text = f.str("text").unwrap_or_default();
            if held.open.as_deref() == Some(message.as_str()) {
                if let Some(last) = held.lines.last_mut() {
                    last.push_str(text);
                }
                held.size += utf16::len(text);
            } else {
                push_line(held, text);
            }
            held.open = Some(message);
        }
        _ => {}
    }
}

/// One thread's words read off its events. internal.ts:436-440.
pub fn thread_words_of(events: &[SessionEvent], thread_id: &str) -> ThreadWords {
    let mut index = empty_index();
    for e in events {
        if Fields::of(e).str("threadId") == Some(thread_id) {
            fold_words(&mut index, e);
        }
    }
    index.words.swap_remove(thread_id).unwrap_or_default()
}

/// A thread's words with its lines let go. internal.ts:443.
pub fn cold_words(w: &ThreadWords) -> ThreadWords {
    ThreadWords {
        lines: Vec::new(),
        open: None,
        last: w.last,
        size: w.size,
        first: w.first.clone(),
        opened: w.opened,
        cold: Some(true),
        room: None,
    }
}

/// A thread's words with its oldest lines let go, its newest kept while they fit in `room`. internal.ts:445-450.
pub fn newest_of(w: &ThreadWords, room: usize) -> ThreadWords {
    if w.cold != Some(true) && w.size <= room {
        return w.clone();
    }
    let mut from = w.lines.len();
    let mut kept = 0;
    while from > 0 && kept + utf16::len(&w.lines[from - 1]) <= room {
        from -= 1;
        kept += utf16::len(&w.lines[from]);
    }
    ThreadWords { lines: w.lines[from..].to_vec(), room: Some(room), ..cold_words(w) }
}

/// The UTF-16 units of words a thread holds now. internal.ts:452.
pub fn held_of(w: &ThreadWords) -> usize {
    if w.cold == Some(true) {
        w.lines.iter().map(|line| utf16::len(line)).sum()
    } else {
        w.size
    }
}

/// internal.ts:454-458.
pub fn index_of(events: &[SessionEvent]) -> TranscriptIndex {
    let mut index = empty_index();
    for e in events {
        fold_event(&mut index, e);
    }
    index
}

/// These threads' entries read again off their own events, in the order named; a session's facts stay. internal.ts:462-466.
pub fn reindex_threads<'a>(
    index: &mut TranscriptIndex,
    threads: impl IntoIterator<Item = &'a String> + Copy,
    events_of: &dyn Fn(&str) -> Vec<SessionEvent>,
) {
    for thread in threads {
        index.words.shift_remove(thread.as_str());
        index.starts.shift_remove(thread.as_str());
        index.cut.shift_remove(thread.as_str());
        index.children.shift_remove(thread.as_str());
    }
    index.taken.retain(|_, t| !threads.into_iter().any(|name| *name == t.thread_id));
    for thread in threads {
        for e in events_of(thread) {
            fold_event(index, &e);
        }
    }
}

/// How much of itself a turn has written. internal.ts:469-482.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct TurnWritten {
    pub lines: u64,
    pub subagents: u64,
    pub compactions: u64,
    pub retries: u64,
    pub reply: Option<TurnStatus>,
    pub started: bool,
    pub changes: bool,
}

/// internal.ts:483-503.
pub fn turn_written(events: &[SessionEvent], turn_id: &str) -> TurnWritten {
    let fields: Vec<Fields> = events.iter().map(Fields::of).collect();
    let last_of = |ty: &str, lined: bool| {
        fields.iter().rev().find(|f| f.str("type") == Some(ty) && f.str("turnId") == Some(turn_id) && (!lined || f.has("line")))
    };
    let count = |ty: &str, lined: bool| last_of(ty, lined).and_then(|f| f.num("line")).unwrap_or(0.0) as u64;
    let lines = count("session.delta", false);
    let subagents = count("session.subagent", false);
    let compactions = count("session.compacted", false);
    let retries = count("session.retry", true);
    let reply = events.iter().rev().find_map(|e| match e {
        SessionEvent::SessionDoneEvent(d) if d.turn_id.as_deref() == Some(turn_id) => Some(d.result.status),
        _ => None,
    });
    let started =
        lines > 0 || subagents > 0 || compactions > 0 || retries > 0 || reply.is_some() || last_of("session.start", false).is_some();
    let changes = last_of("session.changes", false).is_some();
    TurnWritten { lines, subagents, compactions, retries, reply, started, changes }
}

/// The index as it is kept beside a transcript, the JSON of `{of, words, starts, cut, facts, taken, children,
/// trimmed, pos}` with each map as an array of `[key, value]` pairs; with `lines` false each thread's words are
/// `cold_words`. `of` is left out where None. internal.ts:510-511.
pub fn index_bytes(index: &TranscriptIndex, of: Option<BlobMark>, lines: bool) -> Vec<u8> {
    let mut out = serde_json::Map::new();
    if let Some(of) = of {
        out.insert("of".into(), serde_json::to_value(of).unwrap_or(Value::Null));
    }
    let words: Vec<Value> = index
        .words
        .iter()
        .map(|(thread, w)| {
            let w = if lines { w.clone() } else { cold_words(w) };
            json!([thread, w])
        })
        .collect();
    out.insert("words".into(), Value::Array(words));
    out.insert("starts".into(), json!(index.starts.iter().collect::<Vec<_>>()));
    out.insert("cut".into(), json!(index.cut.iter().collect::<Vec<_>>()));
    out.insert("facts".into(), json!(index.facts.iter().collect::<Vec<_>>()));
    out.insert("taken".into(), json!(index.taken.iter().collect::<Vec<_>>()));
    let children: Vec<Value> = index.children.iter().map(|(thread, held)| json!([thread, held.iter().collect::<Vec<_>>()])).collect();
    out.insert("children".into(), Value::Array(children));
    out.insert("trimmed".into(), json!(index.trimmed.iter().collect::<Vec<_>>()));
    out.insert("pos".into(), json!(index.pos));
    serde_json::to_vec(&Value::Object(out)).unwrap_or_default()
}

/// What `index_read` answers.
#[derive(Clone, Debug, PartialEq)]
pub struct IndexRead {
    pub index: TranscriptIndex,
    pub of: Option<BlobMark>,
}

/// Drops the lone surrogate escapes a TS text holds, as `first_of` drops a split pair's half: a Rust string cannot hold one.
fn drop_lone_surrogates(b: &[u8]) -> Vec<u8> {
    let hex_at = |at: usize| b.get(at..at + 4).and_then(|h| u16::from_str_radix(std::str::from_utf8(h).ok()?, 16).ok());
    let u_at = |at: usize| b.get(at..at + 2) == Some(&b"\\u"[..]);
    let mut out = Vec::with_capacity(b.len());
    let mut in_string = false;
    let mut i = 0;
    while i < b.len() {
        let c = b[i];
        if c == b'"' {
            in_string = !in_string;
        }
        if c != b'\\' || !in_string {
            out.push(c);
            i += 1;
        } else if !u_at(i) {
            let end = (i + 2).min(b.len());
            out.extend_from_slice(&b[i..end]);
            i = end;
        } else if matches!(hex_at(i + 2), Some(0xD800..=0xDBFF)) && u_at(i + 6) && matches!(hex_at(i + 8), Some(0xDC00..=0xDFFF)) {
            out.extend_from_slice(&b[i..i + 12]);
            i += 12;
        } else {
            if !matches!(hex_at(i + 2), Some(0xD800..=0xDFFF)) {
                out.extend_from_slice(&b[i..(i + 6).min(b.len())]);
            }
            i += 6;
        }
    }
    out
}

/// An index read back; None where it does not parse or lacks taken, children or pos. internal.ts:512-526.
pub fn index_read(bytes: &[u8]) -> Option<IndexRead> {
    #[derive(Deserialize)]
    struct Held {
        of: Option<BlobMark>,
        words: Vec<(String, Value)>,
        starts: Vec<(String, String)>,
        cut: Vec<(String, bool)>,
        facts: Vec<(String, SessionFacts)>,
        taken: Option<Vec<(String, Taken)>>,
        children: Option<Vec<(String, Vec<(String, Child)>)>>,
        trimmed: Option<Vec<String>>,
        pos: Option<u64>,
    }
    let held: Held = serde_json::from_slice(&drop_lone_surrogates(bytes)).ok()?;
    let (taken, children, pos) = (held.taken?, held.children?, held.pos?);
    let words = held
        .words
        .into_iter()
        .map(|(thread, mut w)| {
            let obj = w.as_object_mut()?;
            let lines: Vec<String> = obj.get("lines").and_then(|l| serde_json::from_value(l.clone()).ok())?;
            if obj.get("size").and_then(Value::as_u64).is_none() {
                obj.insert("size".into(), json!(lines.iter().map(|l| utf16::len(l)).sum::<usize>()));
            }
            if obj.get("first").and_then(Value::as_str).is_none() {
                if let Some(first) = lines.first() {
                    obj.insert("first".into(), json!(first_of(first)));
                }
            }
            Some((thread, serde_json::from_value::<ThreadWords>(w).ok()?))
        })
        .collect::<Option<IndexMap<String, ThreadWords>>>()?;
    let index = TranscriptIndex {
        words,
        starts: held.starts.into_iter().collect(),
        cut: held.cut.into_iter().collect(),
        facts: held.facts.into_iter().collect(),
        taken: taken.into_iter().collect(),
        children: children.into_iter().map(|(thread, kids)| (thread, kids.into_iter().collect())).collect(),
        trimmed: held.trimmed.unwrap_or_default().into_iter().collect(),
        pos,
    };
    Some(IndexRead { index, of: held.of })
}

/// One search hit. internal.ts:528.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchHit {
    pub workspace_id: String,
    pub thread_id: String,
    pub snippet: String,
}

/// internal.ts:528.
#[derive(Clone, Debug, PartialEq)]
pub struct SearchedHit {
    pub hit: SearchHit,
    pub last: JsNumber,
}

/// One folder's hits of `words` (lowercased) into `found`; answers how many threads the caller drives it could not
/// search. internal.ts:532-542.
pub fn search_index(
    workspace_id: &str,
    index: &TranscriptIndex,
    words: &str,
    drives: &dyn Fn(&str) -> bool,
    found: &mut Vec<SearchedHit>,
) -> usize {
    let mut unsearched = 0;
    for (thread_id, w) in &index.words {
        if !drives(thread_id) {
            continue;
        }
        if w.cold == Some(true) && w.lines.is_empty() && w.size > 0 {
            unsearched += 1;
        }
        let Some(text) = w.lines.iter().find(|line| line.to_lowercase().contains(words)) else { continue };
        let lower = text.to_lowercase();
        let at = lower.find(words).map_or(0, |byte| utf16::len(&lower[..byte]));
        let snippet = snippet_around(text, at, utf16::len(words));
        found.push(SearchedHit {
            hit: SearchHit { workspace_id: workspace_id.to_string(), thread_id: thread_id.clone(), snippet },
            last: w.last,
        });
    }
    unsearched
}

/// The words around a hit: 40 UTF-16 units before, 80 after, whitespace runs as one space, trimmed, "…" where the text
/// goes on. internal.ts:544-548.
pub fn snippet_around(text: &str, at: usize, length: usize) -> String {
    let len = utf16::len(text);
    let from = at.saturating_sub(40);
    let to = len.min(at + length + 80);
    let kept = js_trim(&js_collapse_whitespace(&utf16::slice(text, from as i64, Some(to as i64)))).to_string();
    let head = if from > 0 { "\u{2026}" } else { "" };
    let tail = if to < len { "\u{2026}" } else { "" };
    format!("{head}{kept}{tail}")
}

/// internal.ts:551.
pub const SESSION_INDEX_CAP: usize = 200;
/// internal.ts:554.
pub const TRANSCRIPT_FLUSH_MS: u64 = 250;

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
    pub box_file: Option<futures::future::Shared<futures::future::LocalBoxFuture<'static, Option<u64>>>>,
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
