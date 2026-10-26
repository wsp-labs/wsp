//! Unit runs_parts: slate-runs.ts:17-242, 363-489, 985-987 at 65f6b89c7. Spec: spec/runs_parts.md.
//!
//! A run record is an ordered JSON object, not a struct: every road writes its keys in its own TS order (spec
//! Behaviour 1, Decisions 1), and the slate area stores and pushes it as JSON.
use std::future::Future;
use std::pin::Pin;
use std::rc::Rc;

use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use sha2::{Digest, Sha256};
use wsp_wire::types::js_number::JsNumber;
use wsp_words::js::{number_string, utf16};

use crate::slate_read::is_run_record;

/// The six states a run reads.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RunState {
    Idle,
    Held,
    Running,
    Done,
    Failed,
    Cancelled,
}

impl RunState {
    pub fn as_str(self) -> &'static str {
        match self {
            RunState::Idle => "idle",
            RunState::Held => "held",
            RunState::Running => "running",
            RunState::Done => "done",
            RunState::Failed => "failed",
            RunState::Cancelled => "cancelled",
        }
    }
    pub fn parse(s: &str) -> Option<RunState> {
        Some(match s {
            "idle" => RunState::Idle,
            "held" => RunState::Held,
            "running" => RunState::Running,
            "done" => RunState::Done,
            "failed" => RunState::Failed,
            "cancelled" => RunState::Cancelled,
            _ => return None,
        })
    }
}

/// What `$name` reads while and after the run, keys in the order the road that wrote it put them.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct RunRecord(pub Map<String, Value>);

impl RunRecord {
    /// A record from a stored value: an object whose `state` is a string and `runs` a number (slate-read.ts:6).
    pub fn from_json(v: &Value) -> Option<RunRecord> {
        if !is_run_record(Some(v)) {
            return None;
        }
        v.as_object().map(|m| RunRecord(m.clone()))
    }
    pub fn to_json(&self) -> Value {
        Value::Object(self.0.clone())
    }
    /// `state`, when it names one of the six.
    pub fn state(&self) -> Option<RunState> {
        self.get("state").and_then(Value::as_str).and_then(RunState::parse)
    }
    /// `runs`, 0 when absent.
    pub fn runs(&self) -> i64 {
        self.get("runs").and_then(Value::as_f64).map_or(0, |n| n as i64)
    }
    pub fn why(&self) -> Option<&str> {
        self.get("why").and_then(Value::as_str)
    }
    pub fn started_at(&self) -> Option<i64> {
        self.get("startedAt").and_then(Value::as_f64).map(|n| n as i64)
    }
    pub fn get(&self, key: &str) -> Option<&Value> {
        self.0.get(key)
    }
}

/// What a run that ran left for `$name`, the fields always in `RESULT_FIELDS` order.
pub type RunResult = Map<String, Value>;

/// What `$token` reads: `{secret: true, set, len, at}`, `at` null when unset.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SecretHandle {
    pub secret: bool,
    pub set: bool,
    pub len: i64,
    pub at: Option<i64>,
}

/// A cmd run as the slate area hands it over: the declaration with `confirm` as text, the files it reads and the
/// hashes of the scripts it names. `kind` is always `cmd`.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct CmdRunDecl {
    pub cmd: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub env: Option<IndexMap<String, Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub args: Option<Vec<Value>>,
    /// A literal null stdin is kept apart from none (spec Behaviour 7).
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::pending::null_kept::deserialize")]
    pub stdin: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub on: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cwd: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timeout: Option<JsNumber>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stream: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub confirm: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub every: Option<JsNumber>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub always: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub once: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub then: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub files: Option<IndexMap<String, String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scripts: Option<IndexMap<String, String>>,
}

/// One value as it reaches a command; a secret is named, never resolved by the caller.
#[derive(Clone, Debug, PartialEq)]
pub enum RunInput {
    Value(Value),
    Secret(String),
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct RunInputs {
    pub env: Option<IndexMap<String, RunInput>>,
    pub args: Option<Vec<RunInput>>,
    pub stdin: Option<RunInput>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RunBy {
    Person,
    Agent,
    Reaction,
    Timer,
}

impl RunBy {
    pub fn as_str(self) -> &'static str {
        match self {
            RunBy::Person => "person",
            RunBy::Agent => "agent",
            RunBy::Reaction => "reaction",
            RunBy::Timer => "timer",
        }
    }
}

/// A start. `inputs` is read when the command actually starts, which for a held run is when the person approves it.
#[derive(Clone)]
pub struct RunStart {
    pub thread_id: String,
    pub run: String,
    pub decl: CmdRunDecl,
    pub by: RunBy,
    pub folder: String,
    pub inputs: Rc<dyn Fn() -> RunInputs>,
    pub last: Option<RunRecord>,
    pub asleep: bool,
}

/// What the approval sheet shows, keys in the order slate-runs.ts:676-689 writes them.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RunAsk {
    pub run: String,
    pub key: String,
    pub cmd: String,
    pub env: IndexMap<String, String>,
    pub args: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stdin: Option<String>,
    pub folder: String,
    pub on: String,
    pub timeout: JsNumber,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub confirm: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub then: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub files: Option<IndexMap<String, String>>,
}

/// A reshape's answer: the JSON it printed, or why it failed with what it said on stderr.
#[derive(Clone, Debug, PartialEq)]
pub enum ReshapeAnswer {
    Json(Value),
    Failed { why: String, exit: Option<i64>, err: String },
}

/// A reshape under way: `done` settles once; `kill` ends its group.
pub struct Reshape {
    pub done: Pin<Box<dyn Future<Output = ReshapeAnswer>>>,
    pub kill: Rc<dyn Fn()>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Outcome {
    Running,
    Held,
    Noop,
    Failed,
}

#[derive(Clone, Debug, PartialEq)]
pub struct RunStartAnswer {
    pub outcome: Outcome,
    pub record: RunRecord,
    /// Only for a start held for approval, not beside a hold of another kind.
    pub ask: Option<RunAsk>,
}

pub const NO_FOLDER: &str = "this host knows no folder for the thread, so the command did not start";
/// The fields a result keeps, in this order.
pub const RESULT_FIELDS: [&str; 8] = ["exit", "out", "err", "json", "lines", "endedAt", "ms", "cut"];

pub const DEFAULT_TIMEOUT_S: i64 = 60;
pub const LINES_KEPT: usize = 500;
pub const RUNNING_MAX: usize = 4;
pub const STARTS_PER_MINUTE: usize = 12;
pub const TIMER_FLOOR_S: i64 = 10;
pub const BACKOFF_AFTER: i64 = 5;
pub const BACKOFF_MS: i64 = 5 * 60_000;
pub const BACKED_OFF: &str = "five starts in a row failed, so its timer starts it once every 5 minutes until one succeeds";
pub const CLOSE_GRACE_MS: i64 = 1_000;
/// `^[A-Za-z_][A-Za-z0-9_]*$`, ported by hand (rule 8.7).
pub fn is_env_name(name: &str) -> bool {
    let mut chars = name.chars();
    matches!(chars.next(), Some(c) if c.is_ascii_alphabetic() || c == '_') && chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}
pub const SECRET_DOTS: &str = "••••";
pub const HELD_APPROVAL: &str = crate::pending::wire::SLATE_HELD_APPROVAL;
pub const HELD_BUSY: &str = "4 runs are already running";
pub const HELD_BUDGET: &str = "started 12 times in a minute; press to run it again";
pub const HELD_PRESSED: &str = "pressed 12 times in a minute; it can start again in a minute";
pub const HELD_ASLEEP: &str = "the box was asleep, so this tick did not wake it; press to run it now";

/// `the slate's files were not written: <message>`.
pub fn files_unwritten(message: &str) -> String {
    format!("the slate's files were not written: {message}")
}

pub fn last_result(record: &RunRecord) -> Option<RunResult> {
    let ran = match record.state() {
        Some(RunState::Running) => record.get("refreshing") == Some(&Value::Bool(true)),
        Some(RunState::Done | RunState::Failed | RunState::Cancelled) => record.get("startedAt").is_some(),
        _ => false,
    };
    ran.then(|| RESULT_FIELDS.iter().filter_map(|f| record.get(f).map(|v| (f.to_string(), v.clone()))).collect())
}

pub fn running_record(last: Option<&RunResult>, runs: i64, started_at: i64) -> RunRecord {
    let mut out = last.cloned().unwrap_or_default();
    out.insert("state".into(), "running".into());
    out.insert("runs".into(), runs.into());
    out.insert("startedAt".into(), started_at.into());
    if last.is_some() {
        out.insert("refreshing".into(), true.into());
    }
    RunRecord(out)
}

/// `state` is `failed` or `cancelled`.
pub fn ended_record(record: &RunRecord, state: RunState, why: &str, now: i64) -> RunRecord {
    let mut out = Map::new();
    out.insert("state".into(), state.as_str().into());
    out.insert("why".into(), why.into());
    out.insert("exit".into(), Value::Null);
    if let Some(runs) = record.get("runs") {
        out.insert("runs".into(), runs.clone());
    }
    if let Some(started) = record.get("startedAt") {
        out.insert("startedAt".into(), started.clone());
    }
    out.insert("endedAt".into(), now.into());
    RunRecord(out)
}

pub fn restarted_record(record: &RunRecord, now: i64) -> RunRecord {
    if record.state() == Some(RunState::Running) {
        ended_record(record, RunState::Failed, "the host restarted while it ran", now)
    } else {
        record.clone()
    }
}

pub fn rewound_record(record: &RunRecord, now: i64) -> RunRecord {
    if record.state() == Some(RunState::Running) {
        ended_record(record, RunState::Cancelled, "the thread was rewound", now)
    } else {
        record.clone()
    }
}

pub fn spent(times: Option<&[i64]>, at: i64) -> Option<Vec<i64>> {
    let mut kept: Vec<i64> = times.unwrap_or(&[]).iter().copied().filter(|s| at - s < 60_000).collect();
    if kept.len() >= STARTS_PER_MINUTE {
        return None;
    }
    kept.push(at);
    Some(kept)
}

/// `value` none is `undefined`, written `null`.
fn json_string(text: &str) -> String {
    serde_json::to_string(text).expect("a string writes")
}

fn string_map(m: &IndexMap<String, String>) -> Value {
    Value::Object(m.iter().map(|(k, v)| (k.clone(), Value::String(v.clone()))).collect())
}

fn write_json(value: &Value, sorted: bool) -> String {
    match value {
        Value::Array(items) => format!("[{}]", items.iter().map(|v| write_json(v, sorted)).collect::<Vec<_>>().join(",")),
        Value::Object(map) => {
            let mut entries: Vec<(&String, &Value)> = map.iter().collect();
            if sorted {
                entries.sort_by(|a, b| utf16::cmp(a.0, b.0));
            }
            let body: Vec<String> = entries.iter().map(|(k, v)| format!("{}:{}", json_string(k), write_json(v, sorted))).collect();
            format!("{{{}}}", body.join(","))
        }
        Value::Number(n) => number_string(n.as_f64().unwrap_or(f64::NAN)),
        other => serde_json::to_string(other).expect("a value writes"),
    }
}

pub fn stable_json(value: Option<&Value>) -> String {
    value.map_or_else(|| "null".to_string(), |v| write_json(v, true))
}

/// The approval key (the table's `key`).
pub fn run_key(decl: &CmdRunDecl) -> String {
    let mut env: Vec<(&String, &Value)> = decl.env.as_ref().map_or_else(Vec::new, |m| m.iter().collect());
    env.sort_by(|a, b| utf16::cmp(a.0, b.0));
    let env_text: Vec<String> =
        env.iter().map(|(name, expr)| format!("[{},{}]", json_string(name), json_string(&stable_json(Some(expr))))).collect();
    let args: Vec<String> = decl.args.as_deref().unwrap_or(&[]).iter().map(|a| json_string(&stable_json(Some(a)))).collect();
    let stdin = match &decl.stdin {
        None => "null".to_string(),
        Some(v) => json_string(&stable_json(Some(v))),
    };
    let mut what = vec![
        json_string("cmd"),
        json_string(&decl.cmd),
        format!("[{}]", env_text.join(",")),
        format!("[{}]", args.join(",")),
        stdin,
        json_string(decl.on.as_deref().unwrap_or("thread")),
        json_string(decl.cwd.as_deref().unwrap_or("")),
        (decl.stream == Some(true)).to_string(),
        decl.every.map_or_else(|| "null".to_string(), |n| number_string(n.0)),
        (decl.always == Some(true)).to_string(),
        number_string(decl.timeout.map_or(DEFAULT_TIMEOUT_S as f64, |t| t.0)),
        (decl.once == Some(true)).to_string(),
    ];
    if let Some(then) = &decl.then {
        what.push(json_string(then));
    }
    if let Some(files) = &decl.files {
        what.push(json_string(&stable_json(Some(&string_map(files)))));
    }
    if let Some(scripts) = &decl.scripts {
        what.push(json_string(&stable_json(Some(&string_map(scripts)))));
    }
    let digest = Sha256::digest(format!("[{}]", what.join(",")).as_bytes());
    hex::encode(digest)[..32].to_string()
}

pub fn map_strings(value: &Value, f: &dyn Fn(&str) -> String) -> Value {
    match value {
        Value::String(s) => Value::String(f(s)),
        Value::Array(items) => Value::Array(items.iter().map(|v| map_strings(v, f)).collect()),
        Value::Object(map) => Value::Object(map.iter().map(|(k, v)| (k.clone(), map_strings(v, f))).collect()),
        other => other.clone(),
    }
}

pub fn as_text(value: &Value) -> String {
    match value {
        Value::Null => String::new(),
        Value::String(s) => s.clone(),
        Value::Number(n) => number_string(n.as_f64().unwrap_or(f64::NAN)),
        Value::Bool(b) => b.to_string(),
        other => write_json(other, false),
    }
}

/// Seconds, fractions kept.
pub fn timeout_of(decl: &CmdRunDecl) -> f64 {
    decl.timeout.map_or(DEFAULT_TIMEOUT_S as f64, |t| t.0).clamp(1.0, 600.0)
}
