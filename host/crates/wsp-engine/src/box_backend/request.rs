// SPDX-License-Identifier: AGPL-3.0-only
//! box-backend.ts:347-786 at a4033dc34. Spec: spec/box_request.md.
use std::collections::{BTreeSet, HashMap};
use std::sync::{Arc, Mutex};

use crate::cancel::Cancel;
use crate::js_value::js_string;
use futures::FutureExt;
use indexmap::IndexMap;
use serde::Deserialize;
use serde_json::Value;
use wsp_catalog::json_parse::parse_json;

use super::plan::{
    limits_of, place_limits_of, BoxLimits, StartWindow, BACKSTOP_CAP_MS, BOX_API_URL, BOX_CREATE_MS, BOX_PAUSE_MS, BOX_POLL_MS,
    BOX_RESTORE_MS, BOX_RESUME_MS, BOX_SNAPSHOT_MS, LIMITS_KEPT_MS, REFUSAL_LOG_QUIET_MS, TRIAL_TTL_S,
};
use crate::errors::{
    abort, aborted, backoff_ms, classify, road_backoff_ms, road_code, should_retry, EngineError, ErrorKind, RealRetryClock, Result,
    RetryClock, WspError, PROVIDER_READ_CAP_MS, ROAD_TRIES,
};
use crate::fetch::{platform_fetch, Fetch, FetchRequest};
use crate::machine::{CloudLimits, TemplateRow};
use wsp_wire::wire::machine_link::TemplateRowStatus;
use wsp_words::js::{js_round, number_string};
use wsp_words::words::thread::provider_road_retry_line;
use wsp_words::words::units::{
    boat_cancelled_line, boat_plan_words, boat_refused_log_line, boat_request_limit_wait_line, starts_used_line, BOAT_BALANCE_LINE,
    BOAT_MEMBER_CAP_LINE, BOAT_NO_CAPACITY_LINE, BOAT_REQUEST_LIMIT_LINE,
};

/// box-backend.ts:349. A box as the API answers it; every field but `id` and `state` may be absent or null.
#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BoxView {
    #[serde(default = "undefined", deserialize_with = "js_text")]
    pub id: String,
    #[serde(default, deserialize_with = "lenient_string")]
    pub name: Option<String>,
    #[serde(default = "undefined", deserialize_with = "js_text")]
    pub state: String,
    #[serde(default, deserialize_with = "lenient_string")]
    pub error: Option<String>,
    #[serde(default, deserialize_with = "lenient_string")]
    pub last_snapshot_status: Option<String>,
    #[serde(default, deserialize_with = "lenient_string")]
    pub last_snapshot_attempt_at: Option<String>,
    #[serde(default, deserialize_with = "lenient_string")]
    pub snapshot_completed_at: Option<String>,
    #[serde(default, rename = "type", deserialize_with = "lenient_string")]
    pub kind: Option<String>,
    #[serde(default, deserialize_with = "lenient_number")]
    pub vcpu: Option<f64>,
    #[serde(default, rename = "memoryGB", deserialize_with = "lenient_number")]
    pub memory_gb: Option<f64>,
    #[serde(default, deserialize_with = "lenient_string")]
    pub created_at: Option<String>,
    #[serde(default, deserialize_with = "lenient_string")]
    pub archive_after: Option<String>,
    #[serde(default, deserialize_with = "lenient_string")]
    pub subdomain: Option<String>,
}

/// box-backend.ts:368.
#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CommandView {
    #[serde(default, deserialize_with = "lenient_number")]
    pub exit_code: Option<f64>,
    #[serde(default, deserialize_with = "lenient_string")]
    pub stdout: Option<String>,
    #[serde(default, deserialize_with = "lenient_string")]
    pub stderr: Option<String>,
    #[serde(default, deserialize_with = "lenient_bool")]
    pub timed_out: Option<bool>,
}

/// box-backend.ts:375.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NamedSnapshotView {
    #[serde(default = "undefined", deserialize_with = "js_text")]
    pub name: String,
    /// "saving", "ready" or "failed".
    #[serde(default = "undefined", deserialize_with = "js_text")]
    pub status: String,
    #[serde(default, deserialize_with = "lenient_string")]
    pub error: Option<String>,
    #[serde(default, deserialize_with = "lenient_string")]
    pub source_sandbox_id: Option<String>,
    #[serde(default, deserialize_with = "lenient_string")]
    pub snapshot_id: Option<String>,
    #[serde(default, rename = "type", deserialize_with = "lenient_string")]
    pub kind: Option<String>,
    #[serde(default, deserialize_with = "lenient_number")]
    pub size_bytes: Option<f64>,
    #[serde(default, deserialize_with = "lenient_string")]
    pub created_at: Option<String>,
}

/// box-backend.ts:386.
#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SnapshotView {
    #[serde(default = "undefined", deserialize_with = "js_text")]
    pub id: String,
    #[serde(default, deserialize_with = "lenient_string")]
    pub sandbox_id: Option<String>,
    #[serde(default, deserialize_with = "lenient_string")]
    pub status: Option<String>,
    #[serde(default, deserialize_with = "lenient_string")]
    pub kind: Option<String>,
    /// `Some(None)` is a chainId the API answered as null, which the listing passes on as `parent: null`.
    #[serde(default, deserialize_with = "crate::box_backend::request::double_option")]
    pub chain_id: Option<Option<String>>,
    #[serde(default, deserialize_with = "lenient_string")]
    pub created_at: Option<String>,
    #[serde(default, deserialize_with = "lenient_string")]
    pub completed_at: Option<String>,
    #[serde(default, deserialize_with = "lenient_number")]
    pub size_bytes: Option<f64>,
    #[serde(default, deserialize_with = "lenient_number")]
    pub content_size_bytes: Option<f64>,
}

/// A wire view and the fields it reads as text. serde walks a nested value one stack level per level, so `read_view`
/// settles every field that holds a tree before serde sees it.
pub trait View: serde::de::DeserializeOwned {
    const TEXT: &'static [&'static str];
}

impl View for BoxView {
    const TEXT: &'static [&'static str] = &["id", "state"];
}
impl View for CommandView {
    const TEXT: &'static [&'static str] = &[];
}
impl View for NamedSnapshotView {
    const TEXT: &'static [&'static str] = &["name", "status"];
}
impl View for SnapshotView {
    const TEXT: &'static [&'static str] = &["id"];
}

/// The view an answer holds: a field read as text is its JavaScript text, a field the view reads as absent when it is not
/// a scalar (and an array where the TS reads no field) is emptied, so no tree reaches serde, however deep the provider nests it.
pub fn read_view<T: View>(mut answer: Value) -> serde_json::Result<T> {
    match &mut answer {
        Value::Object(fields) => {
            for (key, field) in fields.iter_mut() {
                if !matches!(field, Value::Array(_) | Value::Object(_)) {
                    continue;
                }
                *field = if T::TEXT.contains(&key.as_str()) {
                    Value::String(js_string(field))
                } else if field.is_array() {
                    Value::Array(Vec::new())
                } else {
                    Value::Object(serde_json::Map::new())
                };
            }
        }
        Value::Array(_) => answer = Value::Object(serde_json::Map::new()),
        _ => {}
    }
    serde_json::from_value(answer)
}

/// What a field the TS never checks reads as in a template literal: its JavaScript text, `undefined` where it is absent.
fn js_text<'de, D: serde::Deserializer<'de>>(d: D) -> std::result::Result<String, D::Error> {
    Ok(js_string(&Value::deserialize(d)?))
}

/// `answer.<field>`, moved out of the answer (null where it has none), so a deep tree is never cloned.
pub fn taken(answer: &mut Value, field: &str) -> Value {
    answer.get_mut(field).map(Value::take).unwrap_or(Value::Null)
}

fn undefined() -> String {
    "undefined".to_string()
}

/// A text the TS reads as it stands: a value that is not a string reads as absent, as its `typeof` checks do.
pub fn lenient_string<'de, D: serde::Deserializer<'de>>(d: D) -> std::result::Result<Option<String>, D::Error> {
    Ok(match Value::deserialize(d)? {
        Value::String(s) => Some(s),
        _ => None,
    })
}

/// A flag the TS reads as it stands: only `true` and `false` count.
pub fn lenient_bool<'de, D: serde::Deserializer<'de>>(d: D) -> std::result::Result<Option<bool>, D::Error> {
    Ok(Value::deserialize(d)?.as_bool())
}

/// A size the TS reads as it stands: a value that is not a JSON number reads as absent, as its `typeof` checks do.
pub fn lenient_number<'de, D: serde::Deserializer<'de>>(d: D) -> std::result::Result<Option<f64>, D::Error> {
    Ok(Value::deserialize(d)?.as_f64())
}

/// Reads a present null as `Some(None)` and an absent field as `None`.
pub fn double_option<'de, D: serde::Deserializer<'de>>(d: D) -> std::result::Result<Option<Option<String>>, D::Error> {
    Option::<String>::deserialize(d).map(Some)
}

/// box-backend.ts:409.
pub fn stop_refusal_of(view: &BoxView) -> String {
    if let Some(error) = view.error.as_deref().filter(|e| !e.is_empty()) {
        return error.to_string();
    }
    let attempt = match &view.last_snapshot_status {
        Some(status) => {
            let at = view.last_snapshot_attempt_at.as_deref().map_or(String::new(), |at| format!(" at {at}"));
            format!("its latest snapshot {status}{at}")
        }
        None => "it reported no snapshot attempt".to_string(),
    };
    let completed = view.snapshot_completed_at.as_deref().map_or("none".to_string(), |at| format!("at {at}"));
    format!("{attempt}, and the last that completed was {completed}")
}

/// box-backend.ts:415.
pub fn template_row_of(s: &NamedSnapshotView) -> TemplateRow {
    let status = match s.status.as_str() {
        "ready" => TemplateRowStatus::Ready,
        "failed" => TemplateRowStatus::Failed,
        _ => TemplateRowStatus::Building,
    };
    TemplateRow {
        id: s.name.clone(),
        name: s.name.clone(),
        status,
        error: if s.status == "failed" { s.error.clone() } else { None },
        created_at: s.created_at.clone(),
    }
}

/// box-backend.ts:426. 14 days.
pub const BOX_DISK_KEPT_MS: u64 = 14 * 24 * 3_600_000;
/// box-backend.ts:427.
pub const DISK_EXPIRED_CODE: &str = "snapshot_data_expired";
/// box-backend.ts:440.
pub const REQUEST_LIMIT_CODE: &str = "IP_RATE_LIMIT_EXCEEDED";
/// box-backend.ts:441.
pub const REQUEST_LIMIT_WINDOW_MS: u64 = 60_000;
/// box-backend.ts:443.
pub const REQUEST_LIMIT_TRIES: u32 = 3;

/// box-backend.ts:432. The provider's words for its concurrent-box cap, read as the concurrency kind.
const CAP_CODES: [&str; 2] = ["limit_reached", "member_limit_reached"];
/// box-backend.ts:436. The two codes of a window of starts used.
const START_CODES: [&str; 2] = ["rate_limited", "daily_limit_reached"];
/// box-backend.ts:466 `REFUSAL_LINES`: refusals whose own words name nothing a person can act on.
const REFUSAL_LINES: [(&str, &str); 3] = [
    ("billing_required", BOAT_BALANCE_LINE),
    ("team_member_cap_reached", BOAT_MEMBER_CAP_LINE),
    ("out_of_capacity", BOAT_NO_CAPACITY_LINE),
];

/// `/concurrent (box|sandbox)/i`, which the TS reads off a refusal's message.
fn names_the_concurrent_cap(message: &str) -> bool {
    let lower = message.to_ascii_lowercase();
    lower.contains("concurrent box") || lower.contains("concurrent sandbox")
}

/// box-backend.ts:446, before the milliseconds are whole: `s * 1000`, a number as it comes.
pub fn request_limit_wait_exact_ms(body: &Value) -> f64 {
    let named = match body.get("retryAfter") {
        None | Some(Value::Null) => body.get("error").and_then(|e| e.get("details")).and_then(|d| d.get("retryAfter")),
        top => top,
    };
    match named.and_then(Value::as_f64) {
        Some(s) if s > 0.0 => (s * 1000.0).min(REQUEST_LIMIT_WINDOW_MS as f64),
        _ => REQUEST_LIMIT_WINDOW_MS as f64,
    }
}

/// box-backend.ts:446. `body` is the refusal's parsed JSON, `Value::Null` where it was not JSON.
pub fn request_limit_wait_ms(body: &Value) -> u64 {
    request_limit_wait_exact_ms(body) as u64
}

/// box-backend.ts:459. A refusal read by its code.
pub fn box_refusal(e: WspError) -> WspError {
    if e.code.as_deref() == Some(REQUEST_LIMIT_CODE) {
        return WspError { kind: ErrorKind::Transient, message: BOAT_REQUEST_LIMIT_LINE.to_string(), said: Some(e.message), ..e };
    }
    if let Some(code) = e.code.as_deref().filter(|c| START_CODES.contains(c)) {
        let window = (code == "daily_limit_reached").then_some(StartWindow::Day);
        return WspError { kind: ErrorKind::StartLimit, window: window.or(e.window), ..e };
    }
    let capped = e.code.as_deref().is_some_and(|c| CAP_CODES.contains(&c)) || names_the_concurrent_cap(&e.message);
    if capped {
        return WspError { kind: ErrorKind::Concurrency, ..e };
    }
    if e.status == 429 {
        let said = Some(e.message.clone());
        return WspError { said, ..e };
    }
    match e.code.as_deref().and_then(|c| REFUSAL_LINES.iter().find(|(k, _)| *k == c)).map(|(_, line)| *line) {
        Some(line) => WspError { message: line.to_string(), said: Some(e.message.clone()), ..e },
        None => e,
    }
}

/// box-backend.ts:472. The provider saying the box is not ready for this call yet.
pub fn still_restoring(e: &EngineError) -> bool {
    if e.code() == Some(REQUEST_LIMIT_CODE) {
        return false;
    }
    if e.kind() == Some(ErrorKind::Transient) {
        return true;
    }
    e.kind() == Some(ErrorKind::Conflict) && matches!(e.code(), Some("sandbox_restoring" | "sandbox_starting"))
}

/// box-backend.ts:480. What a call whose answer was lost said.
pub fn road_words(e: &EngineError) -> String {
    let message = e.to_string();
    if let Some(status) = e.status() {
        return format!("{status} {message}");
    }
    match cause_code(e) {
        Some(code) => format!("{message}: {code}"),
        None => message,
    }
}

/// The code of the cause under an error (errors.rs's `cause_code`, which is private there).
fn cause_code(e: &EngineError) -> Option<&str> {
    match e {
        EngineError::Road(r) => r.cause.as_ref().and_then(|c| c.code.as_deref()),
        EngineError::MoveUnanswered { cause, .. } | EngineError::ResumeUnanswered { cause, .. } => {
            cause.as_deref().and_then(EngineError::code)
        }
        EngineError::Unreached { cause, .. } => cause.code(),
        _ => None,
    }
}

/// box-backend.ts:487. A refusal as the error it is thrown as: `EngineError::Provider` whose message is the
/// refusal's, kept even when empty (the TS's fallback words are dead).
pub fn fail(e: WspError) -> EngineError {
    EngineError::Provider(e)
}

pub(super) fn other(message: String) -> EngineError {
    EngineError::Other { message, code: None }
}

/// The error a stopped wait answers: the caller's reason as its message, or the default abort when it passed none. The
/// TS answers a reason that is not an `Error` with `<what> of <id> was stopped by its caller`; a Rust cancel carries
/// words only, so every reason answers as an `Error`'s message does (the coordinator's ruling 4, comment 6405457065).
fn stop_error(signal: &Cancel) -> EngineError {
    match signal.given_reason() {
        None => aborted(),
        Some(reason) => other(reason),
    }
}

pub(super) fn encode_uri_component(text: &str) -> String {
    let mut out = String::new();
    for b in text.bytes() {
        if b.is_ascii_alphanumeric() || b"-_.!~*'()".contains(&b) {
            out.push(char::from(b));
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

/// box-backend.ts:491.
#[derive(Clone, Default)]
pub struct RequestOpts {
    /// Sent as `JSON.stringify(body)`.
    pub body: Option<Value>,
    /// A body already made, sent as these bytes; wins over `body`.
    pub bytes: Option<String>,
    /// In order, after Authorization.
    pub headers: Vec<(String, String)>,
    pub cap_ms: Option<u64>,
    pub signal: Option<Cancel>,
    /// Sent once, for a caller that keeps a backoff of its own.
    pub once: bool,
}

/// box-backend.ts:502.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BoxBudgets {
    pub create_ms: u64,
    pub pause_ms: u64,
    pub resume_ms: u64,
    pub snapshot_ms: u64,
    pub restore_ms: u64,
    pub poll_ms: u64,
    /// The cap on one GET whose road names none.
    pub read_ms: u64,
    pub backstop_ms: u64,
}

/// `Partial<BoxBudgets>`: each `None` takes the measured default.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct BoxBudgetsAsk {
    pub create_ms: Option<u64>,
    pub pause_ms: Option<u64>,
    pub resume_ms: Option<u64>,
    pub snapshot_ms: Option<u64>,
    pub restore_ms: Option<u64>,
    pub poll_ms: Option<u64>,
    pub read_ms: Option<u64>,
    pub backstop_ms: Option<u64>,
}

/// Where `console.warn` goes; tests hand in one that records.
pub type Warn = Arc<dyn Fn(&str)>;

/// box-backend.ts:514.
#[derive(Clone, Default)]
pub struct BoxBackendOptions {
    pub api_key: String,
    /// `None` is [`super::plan::BOX_API_URL`].
    pub base_url: Option<String>,
    /// `None` is the platform's fetch.
    pub fetch: Option<Arc<dyn Fetch>>,
    /// `None` is the real clock.
    pub clock: Option<Arc<dyn RetryClock>>,
    pub budgets: BoxBudgetsAsk,
    /// `None` is stderr.
    pub warn: Option<Warn>,
}

/// The stop timer last sent for a box.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Timer {
    pub until: i64,
    pub span_ms: i64,
}

/// A failed backstop's end and the wait it set.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BackstopFailure {
    pub at: i64,
    pub wait_ms: u64,
}

/// The backend's state across calls (box-backend.ts:572-581). Lives only in memory: a host restart starts it empty.
#[derive(Default)]
pub struct BoxState {
    /// The last answered read of /limits and when it was answered.
    pub limits_kept: Option<(i64, BoxLimits)>,
    /// The read of /limits out now, shared by every caller while it is out.
    pub limits_asked:
        Option<futures::future::WeakShared<futures::future::LocalBoxFuture<'static, std::result::Result<BoxLimits, EngineError>>>>,
    /// When each refusal (call, status, code) was last logged.
    pub refusals_logged: HashMap<String, i64>,
    pub backstops: HashMap<String, Timer>,
    pub backstops_sending: BTreeSet<String>,
    pub backstop_failures: HashMap<String, BackstopFailure>,
}

/// Clears the read of /limits that is out when its future ends, finished or dropped.
struct ClearsLimitsAsked(Arc<BoxShared>);

impl Drop for ClearsLimitsAsked {
    fn drop(&mut self) {
        self.0.state.lock().unwrap().limits_asked = None;
    }
}

/// What a BoxBackend is made of; the handle is cheap to clone and every clone is the same backend.
pub struct BoxShared {
    pub api_key: String,
    pub base_url: String,
    pub fetch: Arc<dyn Fetch>,
    pub clock: Arc<dyn RetryClock>,
    pub budgets: BoxBudgets,
    pub warn: Warn,
    pub state: Mutex<BoxState>,
}

/// box-backend.ts:541.
#[derive(Clone)]
pub struct BoxBackend {
    pub shared: Arc<BoxShared>,
}

impl BoxBackend {
    /// box-backend.ts:583.
    pub fn new(opts: BoxBackendOptions) -> Self {
        let ask = opts.budgets;
        let budgets = BoxBudgets {
            create_ms: ask.create_ms.unwrap_or(BOX_CREATE_MS),
            pause_ms: ask.pause_ms.unwrap_or(BOX_PAUSE_MS),
            resume_ms: ask.resume_ms.unwrap_or(BOX_RESUME_MS),
            snapshot_ms: ask.snapshot_ms.unwrap_or(BOX_SNAPSHOT_MS),
            restore_ms: ask.restore_ms.unwrap_or(BOX_RESTORE_MS),
            poll_ms: ask.poll_ms.unwrap_or(BOX_POLL_MS),
            read_ms: ask.read_ms.unwrap_or(PROVIDER_READ_CAP_MS),
            backstop_ms: ask.backstop_ms.unwrap_or(BACKSTOP_CAP_MS),
        };
        let default_warn: Warn = Arc::new(|line: &str| eprintln!("{line}"));
        let default_clock: Arc<dyn RetryClock> = Arc::new(RealRetryClock);
        Self {
            shared: Arc::new(BoxShared {
                api_key: opts.api_key,
                base_url: opts.base_url.unwrap_or_else(|| BOX_API_URL.to_string()),
                fetch: opts.fetch.unwrap_or_else(platform_fetch),
                clock: opts.clock.unwrap_or(default_clock),
                budgets,
                warn: opts.warn.unwrap_or(default_warn),
                state: Mutex::new(BoxState::default()),
            }),
        }
    }

    pub(super) fn warn(&self, line: &str) {
        (self.shared.warn)(line);
    }

    pub fn budgets(&self) -> BoxBudgets {
        self.shared.budgets
    }

    pub fn clock(&self) -> Arc<dyn RetryClock> {
        self.shared.clock.clone()
    }

    /// box-backend.ts:603. The answer's JSON, `{}` for an empty 2xx body.
    pub async fn request(&self, method: &str, path: &str, opts: RequestOpts) -> Result<Value> {
        let mut headers: IndexMap<String, String> = IndexMap::new();
        headers.insert("Authorization".to_string(), format!("Bearer {}", self.shared.api_key));
        for (name, value) in &opts.headers {
            headers.insert(name.clone(), value.clone());
        }
        let body: Option<Vec<u8>> = match (&opts.bytes, &opts.body) {
            (Some(bytes), _) => Some(bytes.clone().into_bytes()),
            (None, Some(json)) => Some(json.to_string().into_bytes()),
            (None, None) => None,
        };
        if body.is_some() {
            headers.insert("Content-Type".to_string(), "application/json".to_string());
        }
        let headers: Vec<(String, String)> = headers.into_iter().collect();
        let call = format!("{method} {path}");
        let mut road_retries: u32 = 0;
        let mut attempt: u32 = 1;
        let mut limited: u32 = 0;
        loop {
            let cap = opts.cap_ms.or(if method == "GET" { Some(self.shared.budgets.read_ms) } else { None });
            let req = FetchRequest {
                method: method.to_string(),
                url: format!("{}{path}", self.shared.base_url),
                headers: headers.clone(),
                body: body.clone(),
                abort: abort(cap.map(|ms| ms as f64), opts.signal.as_ref()),
            };
            let res = match self.shared.fetch.fetch(req).await {
                Ok(res) => res,
                Err(road) => {
                    let e = EngineError::Road(road);
                    let Some(code) = road_code(&e) else { return Err(e) };
                    road_retries += 1;
                    if road_retries >= if opts.once { 1 } else { ROAD_TRIES } {
                        return Err(e);
                    }
                    self.warn(&provider_road_retry_line(&call, &code, f64::from(road_retries + 1), f64::from(ROAD_TRIES)));
                    self.shared.clock.sleep(road_backoff_ms(road_retries, self.shared.clock.random())).await;
                    continue;
                }
            };
            let text = res.text();
            if res.ok() {
                return if text.is_empty() {
                    Ok(Value::Object(serde_json::Map::new()))
                } else {
                    parse_json(&text).map_err(|e| other(e.to_string()))
                };
            }
            let envelope = parse_json(&text).unwrap_or_else(|_| Value::Object(serde_json::Map::new()));
            if envelope.is_null() {
                return Err(other("Cannot read properties of null (reading 'code')".to_string()));
            }
            let error = envelope.get("error");
            let code = envelope
                .get("code")
                .and_then(Value::as_str)
                .or_else(|| error.and_then(|e| e.get("code")).and_then(Value::as_str))
                .map(str::to_string);
            let said = envelope
                .get("message")
                .and_then(Value::as_str)
                .or_else(|| error.and_then(|e| e.get("message")).and_then(Value::as_str))
                .unwrap_or(&text)
                .to_string();
            let request_id = envelope.get("requestId").and_then(Value::as_str).map(str::to_string);
            self.log_refusal(&call, res.status, code.as_deref(), request_id.as_deref(), &said);
            let e = box_refusal(classify(res.status, code.as_deref(), Some(&said), request_id.as_deref()));
            if e.code.as_deref() == Some(REQUEST_LIMIT_CODE) {
                limited += 1;
                if opts.once || limited >= REQUEST_LIMIT_TRIES {
                    return Err(fail(e));
                }
                let wait = request_limit_wait_exact_ms(&envelope);
                self.warn(&boat_request_limit_wait_line(&call, wait, f64::from(limited + 1), f64::from(REQUEST_LIMIT_TRIES)));
                self.shared.clock.sleep(wait as u64).await;
                if let Some(stopped) = opts.signal.as_ref().filter(|c| c.is_cancelled()) {
                    return Err(stop_error(stopped));
                }
                continue;
            }
            if opts.once || !should_retry(&e, attempt) {
                return Err(fail(e));
            }
            self.shared.clock.sleep(backoff_ms(attempt, self.shared.clock.random())).await;
            attempt += 1;
        }
    }

    /// box-backend.ts:653.
    pub fn log_refusal(&self, call: &str, status: u16, code: Option<&str>, request_id: Option<&str>, said: &str) {
        let key = format!("{call} {status} {}", code.unwrap_or(""));
        let now = self.shared.clock.now();
        let mut state = self.shared.state.lock().unwrap();
        if state.refusals_logged.get(&key).is_some_and(|at| now - at < REFUSAL_LOG_QUIET_MS as i64) {
            return;
        }
        state.refusals_logged.retain(|_, at| now - *at < REFUSAL_LOG_QUIET_MS as i64);
        state.refusals_logged.insert(key, now);
        drop(state);
        self.warn(&boat_refused_log_line(call, f64::from(status), code, request_id, said));
    }

    /// box-backend.ts:664.
    pub async fn limits(&self) -> Result<BoxLimits> {
        let kept = self.shared.state.lock().unwrap().limits_kept.clone();
        match kept {
            Some((at, limits)) if self.shared.clock.now() - at < LIMITS_KEPT_MS as i64 => Ok(limits),
            _ => self.read_limits().await,
        }
    }

    /// box-backend.ts:671. `None` where the read failed.
    pub async fn place_limits(&self) -> Option<CloudLimits> {
        place_limits_of(&self.limits().await.ok()?)
    }

    /// box-backend.ts:677.
    pub async fn read_limits(&self) -> Result<BoxLimits> {
        let asked = {
            let mut state = self.shared.state.lock().unwrap();
            match state.limits_asked.as_ref().and_then(|weak| weak.upgrade()) {
                Some(asked) => asked,
                None => {
                    let this = self.clone();
                    let read = async move {
                        // A fetch that answers without yielding would end before a second caller is polled; the TS's
                        // promise is out from the call, so the second caller joins it.
                        tokio::task::yield_now().await;
                        // The TS's promise runs on and its `finally` clears the guard; here a read every caller has
                        // stopped waiting on is dropped, and the guard clears with it.
                        let _clears = ClearsLimitsAsked(this.shared.clone());
                        this.fetch_limits().await
                    };
                    let asked = read.boxed_local().shared();
                    state.limits_asked = asked.downgrade();
                    asked
                }
            }
        };
        asked.await
    }

    async fn fetch_limits(&self) -> Result<BoxLimits> {
        let raw = self.request("GET", "/limits", RequestOpts::default()).await?;
        let limits = limits_of(&raw);
        let now = self.shared.clock.now();
        self.shared.state.lock().unwrap().limits_kept = Some((now, limits.clone()));
        Ok(limits)
    }

    /// box-backend.ts:692.
    pub async fn before_start(&self) -> Result<BoxLimits> {
        match self.limits().await {
            Ok(limits) => Ok(limits),
            Err(e) => {
                let kept = self.shared.state.lock().unwrap().limits_kept.clone();
                match kept {
                    Some((_, limits)) => Ok(limits),
                    None => Err(e),
                }
            }
        }
    }

    /// box-backend.ts:702.
    pub fn counted(&self) {
        let mut state = self.shared.state.lock().unwrap();
        if let Some((_, limits)) = state.limits_kept.as_mut() {
            if let Some(starts) = limits.starts.as_mut() {
                starts.minute.used += 1.0;
                starts.hour.used += 1.0;
                starts.day.used += 1.0;
            }
        }
    }

    /// box-backend.ts:709. The error a refused start is thrown as: `e` itself, or the start window's line.
    pub async fn start_refused(&self, e: EngineError, what: Option<&str>) -> EngineError {
        let EngineError::Provider(refused) = &e else { return e };
        if refused.kind != ErrorKind::StartLimit {
            return e;
        }
        let Ok(limits) = self.read_limits().await else { return e };
        let Some(starts) = limits.starts else { return e };
        let spent = |w: StartWindow| {
            let c = starts.get(w);
            c.used >= c.limit
        };
        if refused.window.is_none() && !spent(StartWindow::Hour) && !spent(StartWindow::Minute) {
            return e;
        }
        let window = refused.window.unwrap_or(if spent(StartWindow::Hour) { StartWindow::Hour } else { StartWindow::Minute });
        let count = starts.get(window);
        let plan = boat_plan_words(limits.trial, limits.plan_usd);
        let line = starts_used_line(window, count.used, count.limit, plan.as_deref(), what);
        EngineError::Provider(WspError {
            kind: refused.kind,
            status: refused.status,
            code: refused.code.clone(),
            message: line,
            request_id: refused.request_id.clone(),
            window: Some(window),
            said: Some(refused.message.clone()),
        })
    }

    /// box-backend.ts:726. `Some(7200)` on the trial, `None` once it has paid.
    pub async fn ttl_cap(&self) -> Result<Option<u64>> {
        let kept = self.shared.state.lock().unwrap().limits_kept.as_ref().map(|(_, limits)| limits.trial);
        let trial = match kept {
            Some(trial) => trial,
            None => self.read_limits().await?.trial,
        };
        Ok(trial.then_some(TRIAL_TTL_S))
    }

    /// box-backend.ts:733. `instant` in ms since the epoch on the backend's clock.
    pub async fn ttl_seconds_for(&self, instant: Option<i64>) -> Result<Option<u64>> {
        self.ttl_seconds_until(instant.map(|at| at as f64)).await
    }

    /// `ttl_seconds_for` with the instant a JavaScript number holds it: an idle window past an i64 is not a crash.
    pub async fn ttl_seconds_until(&self, instant: Option<f64>) -> Result<Option<u64>> {
        let cap = self.ttl_cap().await?;
        let Some(instant) = instant else { return Ok(cap) };
        let seconds = ((instant - self.shared.clock.now() as f64) / 1000.0).ceil().max(1.0) as u64;
        Ok(Some(cap.map_or(seconds, |cap| seconds.min(cap))))
    }

    /// box-backend.ts:742.
    pub fn last_timer(&self, id: &str) -> Option<Timer> {
        self.shared.state.lock().unwrap().backstops.get(id).copied()
    }

    /// box-backend.ts:751.
    pub async fn view(&self, id: &str) -> Result<BoxView> {
        let mut raw = self.request("GET", &format!("/sandboxes/{}", encode_uri_component(id)), RequestOpts::default()).await?;
        read_view(taken(&mut raw, "sandbox")).map_err(|e| other(e.to_string()))
    }

    /// box-backend.ts:757. Reads the box until `done`, at the poll pace, inside `budget_ms`.
    #[allow(clippy::too_many_arguments)]
    pub async fn settle(
        &self,
        id: &str,
        done: &dyn Fn(&BoxView) -> bool,
        budget_ms: u64,
        what: &str,
        signal: Option<&Cancel>,
        name: Option<&str>,
        seen: Option<&dyn Fn(&BoxView)>,
    ) -> Result<BoxView> {
        let started = self.shared.clock.now();
        loop {
            if let Some(stopped) = signal.filter(|c| c.is_cancelled()) {
                return Err(stop_error(stopped));
            }
            let view = self.view(id).await?;
            if let Some(seen) = seen {
                seen(&view);
            }
            if done(&view) {
                return Ok(view);
            }
            if view.state == "error" {
                let suffix = view.error.as_deref().filter(|e| !e.is_empty()).map_or(String::new(), |e| format!(": {e}"));
                return Err(other(format!("box {id} reads error during {what}{suffix}")));
            }
            if view.state == "cancelled" {
                return Err(EngineError::CreateCancelled {
                    machine_id: id.to_string(),
                    why: view.error.clone().unwrap_or_default(),
                    line: boat_cancelled_line(name, view.error.as_deref()),
                });
            }
            if self.shared.clock.now() - started >= budget_ms as i64 {
                let after = number_string(js_round(budget_ms as f64 / 1000.0));
                return Err(other(format!("{what} of {id} still reads {} after {after} s", view.state)));
            }
            self.shared.clock.sleep(self.shared.budgets.poll_ms).await;
        }
    }

    /// box-backend.ts:774. Asks `call` again while the box is still restoring, inside the restore budget.
    pub async fn while_restoring<T, F, Fut>(&self, mut call: F, signal: Option<&Cancel>) -> Result<T>
    where
        F: FnMut() -> Fut,
        Fut: std::future::Future<Output = Result<T>>,
    {
        let started = self.shared.clock.now();
        loop {
            if let Some(stopped) = signal.filter(|c| c.is_cancelled()) {
                return Err(stop_error(stopped));
            }
            match call().await {
                Ok(answer) => return Ok(answer),
                Err(e) => {
                    if !still_restoring(&e) || self.shared.clock.now() - started >= self.shared.budgets.restore_ms as i64 {
                        return Err(e);
                    }
                    self.shared.clock.sleep(self.shared.budgets.poll_ms).await;
                }
            }
        }
    }
}
