// SPDX-License-Identifier: AGPL-3.0-only
//! Unit slates_model: slates.ts:81-292, 686-758 at 65f6b89c7. Spec: spec/slates_model.md.
use std::collections::{HashSet, VecDeque};
use std::rc::Rc;
use std::sync::OnceLock;

use indexmap::IndexMap;
use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use wsp_words::exit::{HostError, Kind};

use super::{DeliveryOutcome, SlatesInner};
use crate::pending::slate::{slate_run_idle, SlateDoc, SlateProblem, SlateRunDecl, SlateValues};
use crate::pending::wire::SlateEmpty;
use crate::slate_mcp::StopAll;
use crate::slate_runs::{rewound_record, RunRecord, RunState, SecretHandle, TimerDecl};

// slates.ts:81
// /** The slate's content at one version: what a snapshot, the undo and a rewind keep. */
// interface SlateSnap {
//   document: SlateDoc | null;
//   values: SlateValues;
//   empty?: SlateEmpty;
// }
/// The slate's content at one version.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SlateSnap {
    pub document: Option<SlateDoc>,
    pub values: SlateValues,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub empty: Option<SlateEmpty>,
}

// slates.ts:88
// /** One approval the person gave, by the run declaration's key; a refusal is kept the same way. */
// interface Approval {
//   state: "allowed" | "refused";
//   at: number;
//   run?: string;
//   cmd?: string;
//   /** The named files' hashes the person allowed, so a run that asks again can say which one changed. */
//   scripts?: Record<string, string>;
// }
/// One approval the person gave, by key; a refusal is kept the same way.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Approval {
    /// `allowed` or `refused`.
    pub state: String,
    pub at: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub run: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cmd: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scripts: Option<IndexMap<String, String>>,
}

/// Where a turn ended: `{at, version, revision}`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TurnMark {
    pub at: i64,
    pub version: i64,
    pub revision: i64,
}

/// `previous`: the snapshot and the version it was.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PreviousSnap {
    pub document: Option<SlateDoc>,
    pub values: SlateValues,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub empty: Option<SlateEmpty>,
    pub version: i64,
}

// slates.ts:98
// /** One thread's slate as the store keeps it. Snapshots are kept by version and turns point at them, so a turn in
//  * which nothing about the slate changed adds a pointer and no copy. Secrets are handles here, never plaintext. */
// export interface SlateRecord extends SlateSnap {
//   threadId: string;
//   workspaceId: string;
//   rootThreadId: string;
//   schema: 2;
//   /** Moves only when the document does. */
//   version: number;
//   /** Rises with every batch that moved a value and with every version; orders the pushes, checked by nothing. */
//   revision: number;
//   previous?: SlateSnap & { version: number };
//   /** By turn id, in the order the turns ended; kept is by revision, since values move without the version. */
//   turns: Record<string, { at: number; version: number; revision: number }>;
//   kept: Record<string, SlateSnap>;
//   comments: Record<string, SlateJson>[];
//   /** By approval key; "Always in this thread", "Allow" and "Not now" stay, "Run once" is never kept. */
//   approvals: Record<string, Approval>;
//   /** When the person let this slate message the agent from a reaction (07, "A slate that messages the agent"). */
//   sendsAllowed?: number;
//   /** Runtime problems the batch and the runs left standing, newest last. */
//   problems: SlateProblem[];
//   shownOnce: boolean;
//   updatedAt: number;
// }
/// One thread's slate as the store keeps it, keys in `fresh_record`'s order (spec Behaviour 1).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SlateRecord {
    pub thread_id: String,
    pub workspace_id: String,
    pub root_thread_id: String,
    pub schema: i64,
    pub version: i64,
    /// Absent only in a stored record of before revisions; read back as `revision ?? version` (slates.ts:433).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub revision: Option<i64>,
    pub document: Option<SlateDoc>,
    pub values: SlateValues,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub empty: Option<SlateEmpty>,
    pub turns: IndexMap<String, TurnMark>,
    pub kept: IndexMap<String, SlateSnap>,
    pub comments: Vec<Value>,
    pub approvals: IndexMap<String, Approval>,
    pub problems: Vec<SlateProblem>,
    pub shown_once: bool,
    pub updated_at: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub previous: Option<PreviousSnap>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sends_allowed: Option<i64>,
}

impl SlateRecord {
    /// `revision`, read as `version` where a stored record has none.
    pub fn rev(&self) -> i64 {
        self.revision.unwrap_or(self.version)
    }
}

// slates.ts:199
// type BatchBy = "person" | "agent" | "reaction" | "run" | "timer" | "host";
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BatchBy {
    Person,
    Agent,
    Reaction,
    Run,
    Timer,
    Host,
}

// slates.ts:210
// const SNAPSHOTS_KEPT = 100;
// const SNAPSHOT_BYTES = 8 * 1024 * 1024;
// const MESSAGE_CHARS = 20_000;
// const REQUEST_KEPT_MS = 10 * 60_000;
// /** How many presses a request id is remembered for at most, the oldest dropped first. */
// const REQUESTS_KEPT = 500;
// /** The person's events a slate takes: five a second, a burst of five, before any is remembered or batched. */
// const EVENTS_PER_SECOND = 5;
// const PRESS_SEND_MS = 2_000;
// const REACTION_SEND_MS = 60_000 / SLATE_LIMITS.reactionSendsPerMinute;
// const PROBLEMS_KEPT = 20;
// const PIECES_NAMED = SLATE_LIMITS.piecesNamed;
pub const SNAPSHOTS_KEPT: usize = 100;
pub const SNAPSHOT_BYTES: usize = 8 * 1024 * 1024;
pub const MESSAGE_CHARS: usize = 20_000;
pub const REQUEST_KEPT_MS: i64 = 10 * 60_000;
pub const REQUESTS_KEPT: usize = 500;
pub const EVENTS_PER_SECOND: f64 = 5.0;
pub const PRESS_SEND_MS: i64 = 2_000;
pub const REACTION_SEND_MS: i64 = 60_000;
pub const PROBLEMS_KEPT: usize = 20;
pub const PIECES_NAMED: usize = 20;
pub use crate::pending::wire::SLATE_SEND_KEY;

/// `{code, name, message, ...extra}`; the extra here is the fix.
pub fn problem(code: &str, name: &str, message: &str, fix: Option<&str>) -> SlateProblem {
    SlateProblem { code: code.into(), name: name.into(), message: message.into(), fix: fix.map(String::from), ..Default::default() }
}

/// `<code> <name>: <message>` with this kind and the code.
pub fn refused(p: &SlateProblem, kind: Kind) -> HostError {
    let mut e = HostError::with_kind(format!("{} {}: {}", p.code, p.name, p.message), kind);
    e.code = Some(p.code.clone());
    e
}

pub fn usage(p: &SlateProblem) -> HostError {
    refused(p, Kind::Usage)
}

const SOURCE_NAMES: [&str; 6] = ["thread", "usage", "cost", "time", "git", "pr"];

fn source_patterns() -> &'static [(&'static str, Regex)] {
    static PATTERNS: OnceLock<Vec<(&'static str, Regex)>> = OnceLock::new();
    PATTERNS.get_or_init(|| {
        SOURCE_NAMES.iter().map(|name| (*name, Regex::new(&format!(r"(?-u:\b){name}\.")).expect("a source name pattern"))).collect()
    })
}

/// The source names a document or a list of paths reads, read off its text.
pub fn sources_named(text: &str) -> Vec<String> {
    source_patterns().iter().filter(|(_, re)| re.is_match(text)).map(|(name, _)| name.to_string()).collect()
}

/// A value path as written, `$` optional: `i` is `$i`.
pub fn own_path(path: &str) -> String {
    if path.starts_with('$') {
        path.to_string()
    } else {
        format!("${path}")
    }
}

/// The declaration as JSON less `every`, `always`, `timeout`, `stream`.
pub fn command_of(decl: &SlateRunDecl) -> Value {
    match serde_json::to_value(decl) {
        Ok(Value::Object(map)) => {
            Value::Object(map.into_iter().filter(|(key, _)| !matches!(key.as_str(), "every" | "always" | "timeout" | "stream")).collect())
        }
        Ok(other) => other,
        Err(_) => Value::Null,
    }
}

/// `kind` is `send`, `steer` or `queue`; `outcome` `started`, `steered` or `queued`.
pub fn said(kind: &str, outcome: DeliveryOutcome) -> &'static str {
    use DeliveryOutcome::{Queued, Started, Steered};
    match (kind, outcome) {
        ("steer", Started) => "Nothing was running, so it was sent as the next message",
        ("steer", Steered) => "Sent into the running turn",
        ("queue", Started) => "Sent",
        ("queue", Steered) => "Sent into the running turn; this agent takes messages mid-turn",
        (_, Started) => "Sent",
        (_, Steered) => "Sent into the running turn",
        (_, Queued) => "Waiting for the turn to end",
    }
}

/// UTF-8 bytes of the JSON text.
pub fn size_of(value: &Value) -> usize {
    serde_json::to_string(value).map_or(0, |text| text.len())
}

fn kept_bytes(kept: &IndexMap<String, SlateSnap>) -> usize {
    serde_json::to_vec(kept).map_or(0, |bytes| bytes.len())
}

fn drop_unused_snapshots(r: &mut SlateRecord) {
    let used: HashSet<String> = r.turns.values().map(|mark| mark.revision.to_string()).collect();
    r.kept.retain(|revision, _| used.contains(revision));
}

/// Drops the oldest snapshots past the count and the byte cap, then every kept version no turn points at.
pub fn trim_snapshots(r: &mut SlateRecord) {
    let mut order: VecDeque<String> = r.turns.keys().cloned().collect();
    while order.len() > SNAPSHOTS_KEPT {
        if let Some(turn) = order.pop_front() {
            r.turns.shift_remove(&turn);
        }
    }
    drop_unused_snapshots(r);
    while order.len() > 1 && kept_bytes(&r.kept) > SNAPSHOT_BYTES {
        if let Some(turn) = order.pop_front() {
            r.turns.shift_remove(&turn);
        }
        drop_unused_snapshots(r);
    }
}

pub fn snap_of(r: &SlateRecord) -> SlateSnap {
    SlateSnap { document: r.document.clone(), values: r.values.clone(), empty: r.empty }
}

fn secret_value(handle: SecretHandle) -> Value {
    serde_json::to_value(handle).unwrap_or(Value::Null)
}

fn every_of(decl: &SlateRunDecl) -> Option<f64> {
    match decl {
        SlateRunDecl::Cmd(cmd) => cmd.every.as_ref().map(|every| every.0),
        SlateRunDecl::Tool(tool) => tool.every.as_ref().map(|every| every.0),
        SlateRunDecl::Resource(resource) => resource.every.as_ref().map(|every| every.0),
    }
}

fn always_of(decl: &SlateRunDecl) -> bool {
    match decl {
        SlateRunDecl::Cmd(cmd) => cmd.always == Some(true),
        SlateRunDecl::Tool(tool) => tool.always == Some(true),
        SlateRunDecl::Resource(resource) => resource.always == Some(true),
    }
}

fn server_of(decl: &SlateRunDecl) -> Option<&str> {
    match decl {
        SlateRunDecl::Cmd(_) => None,
        SlateRunDecl::Tool(tool) => Some(&tool.server),
        SlateRunDecl::Resource(resource) => Some(&resource.server),
    }
}

impl SlatesInner {
    pub(crate) fn refresh_secrets(&self, r: &mut SlateRecord) {
        let Some(doc) = r.document.as_ref() else { return };
        let secrets = self.runs().secrets();
        for (name, decl) in &doc.values {
            if decl.secret == Some(true) {
                r.values.insert(name.clone(), secret_value(secrets.handle(&r.thread_id, name)));
            }
        }
    }

    pub(crate) fn arm_timers(self: &Rc<Self>, r: &SlateRecord, stopped: &HashSet<String>) {
        let runs = self.runs();
        let mut timers = Vec::new();
        let mut servers: Vec<(String, bool)> = Vec::new();
        if let Some(doc) = &r.document {
            for (run, decl) in &doc.runs {
                let Some(every) = every_of(decl) else { continue };
                if stopped.contains(run) {
                    continue;
                }
                let key = match decl {
                    SlateRunDecl::Cmd(_) => runs.key(&self.approval_decl(r, decl)),
                    _ => serde_json::to_string(decl).unwrap_or_default(),
                };
                timers.push(TimerDecl { run: run.clone(), every, key, always: always_of(decl) });
            }
            for decl in doc.runs.values() {
                if let Some(server) = server_of(decl) {
                    servers.push((server.to_string(), every_of(decl).is_some() && always_of(decl)));
                }
            }
        }
        runs.timers(&r.thread_id, timers);
        let names: Vec<String> = servers.iter().map(|(server, _)| server.clone()).collect();
        let kept: Vec<String> = servers.iter().filter(|(_, kept)| *kept).map(|(server, _)| server.clone()).collect();
        self.mcp().servers(&r.thread_id, &names, &kept);
    }

    /// The TS `become`.
    pub(crate) fn become_snap(self: &Rc<Self>, r: &mut SlateRecord, snap: SlateSnap) {
        let thread_id = r.thread_id.clone();
        self.runs().stop_all(&thread_id, true, None);
        self.mcp().stop_all(&thread_id, StopAll { quiet: true, why: None });
        let no_document = snap.document.is_none();
        let empty = snap.empty;
        r.document = snap.document;
        r.values = snap.values;
        let now = self.deps.clock().now();
        if let Some(doc) = &r.document {
            for name in doc.runs.keys() {
                let rewound = r.values.get(name).and_then(RunRecord::from_json).map(|record| rewound_record(&record, now));
                if let Some(record) = rewound {
                    r.values.insert(name.clone(), record.to_json());
                }
            }
        }
        self.refresh_secrets(r);
        r.empty = if no_document { Some(empty.unwrap_or(SlateEmpty::None)) } else { None };
        self.arm_timers(r, &HashSet::new());
    }

    pub(crate) fn values_for(&self, thread_id: &str, before: Option<&SlateDoc>, values: &SlateValues, next: &SlateDoc) -> SlateValues {
        let secrets = self.runs().secrets();
        let mut live = SlateValues::new();
        for (name, decl) in &next.values {
            if decl.secret == Some(true) {
                live.insert(name.clone(), secret_value(secrets.handle(thread_id, name)));
            } else if before.is_some_and(|old| old.values.contains_key(name)) && values.contains_key(name) {
                live.insert(name.clone(), values[name].clone());
            } else {
                live.insert(name.clone(), decl.start.clone());
            }
        }
        for (name, decl) in &next.runs {
            let was = before.and_then(|old| old.runs.get(name));
            match (was, values.get(name).and_then(RunRecord::from_json)) {
                (Some(was), Some(mut record)) => {
                    let finished = matches!(record.state(), Some(RunState::Done | RunState::Failed | RunState::Cancelled));
                    if finished && command_of(was) != command_of(decl) {
                        record.0.insert("stale".into(), Value::Bool(true));
                    }
                    live.insert(name.clone(), record.to_json());
                }
                _ => {
                    live.insert(name.clone(), slate_run_idle());
                }
            }
        }
        live
    }

    pub(crate) fn drop_declared(&self, r: &SlateRecord, next: Option<&SlateDoc>) {
        let Some(old) = r.document.as_ref() else { return };
        let secrets = self.runs().secrets();
        for (name, decl) in &old.values {
            let still_secret = next.and_then(|doc| doc.values.get(name)).is_some_and(|d| d.secret == Some(true));
            if decl.secret == Some(true) && !still_secret {
                secrets.forget(&r.thread_id, Some(name));
            }
        }
        for name in old.runs.keys() {
            if next.is_some_and(|doc| doc.runs.contains_key(name)) {
                continue;
            }
            self.runs().cancel(&r.thread_id, name);
            self.mcp().cancel(&r.thread_id, name);
        }
    }
}
