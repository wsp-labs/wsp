// SPDX-License-Identifier: AGPL-3.0-only
//! Port of packages/engine/src/solari-backend.ts. Specs: spec/solari_call.md (the backend, its one HTTP call, create,
//! the listings) and spec/solari_machine.md (a machine's exec, pause, resume and reads).
use std::sync::Arc;

use crate::cancel::Cancel;
use async_trait::async_trait;
use indexmap::IndexMap;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Deserializer};
use serde_json::{json, Map, Value};

use crate::errors::{
    abort, backoff_ms, classify, is_capped, is_missing, is_network_error, road_backoff_ms, road_code, should_retry, EngineError, ErrorKind,
    RealRetryClock, Result, RetryClock, WspError, PROVIDER_READ_CAP_MS, ROAD_TRIES,
};
use crate::exec_detached::{exec_detached, INLINE_EXEC_MS};
use crate::fetch::{platform_fetch, Fetch, FetchRequest, FetchResponse};
use crate::golden_seam::EXEC_ENV;
use crate::js_value::js_string;
use crate::machine::{
    BackendOffers, BackendPricing, Capabilities, ExecOptions, ExecResult, Lifecycle, LifecycleBudgets, Machine, MachineBackend,
    MachineKind, MachineLife, MachineListRow, MachineOffers, MachineShape, MachineSpec, MachineState, OnLine, PreviewReach, RunOptions,
    Seen, Size, SnapshotOptions, SnapshotRow, SnapshotStoragePricing, TemplateRow, Waiting, WireSnapshotStoragePricing,
};
use crate::tool_sizes::BUILDER_DISK_GB;
use wsp_catalog::json_parse::parse_json;
use wsp_wire::types::js_number::JsInt;
use wsp_wire::types::JsNumber;
use wsp_wire::wire::capabilities::{MachineSizeOffer, PauseMode, WorkspaceSize};
use wsp_wire::wire::machine_link::{LifecycleBudgetsResumeAsks, TemplateRowStatus};
use wsp_words::js::{js_trim, utf16};
use wsp_words::place_word::provider_key_name;
use wsp_words::words::computer::{move_timed_out_line, Move, RESUME_UNANSWERED};
use wsp_words::words::thread::{
    image_served_wait_line, machine_unreachable_line, provider_road_retry_line, snapshot_listed_refused_line, snapshot_listed_wait_line,
};

/// solari-backend.ts:22. The budgets a pause and a resume run on, in milliseconds.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MoveBudgets {
    pub pause_ms: f64,
    pub resume_cap_ms: f64,
    pub state_read_ms: f64,
    /// The cap on one GET whose road names none.
    pub read_ms: f64,
}

/// solari-backend.ts:18 `budgets`: each one absent takes its shipped default.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct MoveBudgetsGiven {
    pub pause_ms: Option<f64>,
    pub resume_cap_ms: Option<f64>,
    pub state_read_ms: Option<f64>,
    pub read_ms: Option<f64>,
}

/// solari-backend.ts:10.
#[derive(Clone, Default)]
pub struct SolariBackendOptions {
    pub api_key: String,
    /// `https://api.getsolari.com` when `None`.
    pub base_url: Option<String>,
    /// The platform's fetch when `None`.
    pub fetch: Option<Arc<dyn Fetch>>,
    /// `RealRetryClock` when `None`.
    pub clock: Option<Arc<dyn RetryClock>>,
    pub budgets: MoveBudgetsGiven,
    /// Where the TS `console.warn` lines go, one call per line; stderr when `None`.
    pub warn: Option<OnLine>,
}

/// solari-backend.ts:69. Billed from 2026-10-01, 10 GB free, then $0.05 per GB-month.
pub fn snapshot_storage_pricing() -> SnapshotStoragePricing {
    SnapshotStoragePricing {
        wire: WireSnapshotStoragePricing { free_gb: JsNumber(10.0), usd_per_gb_month: JsNumber(0.05), billed_from: "2026-10-01".into() },
        free_named: None,
        usd_per_named_month: None,
    }
}

/// solari-backend.ts:72. The header a request id is read off, should one appear.
pub const REQUEST_ID_HEADER: &str = "x-request-id";

/// solari-backend.ts:75. `cpu * 0.035 + (mem_mb / 1024) * 0.01` per hour.
pub fn rate_usd_per_hour(size: Size) -> f64 {
    size.cpu * 0.035 + (size.mem_mb / 1024.0) * 0.01
}

/// solari-backend.ts:80. The one size a fork has been measured coming up at, and the only one offered.
pub const DEFAULT_SIZE: Size = Size { cpu: 2.0, mem_mb: 4096.0 };

/// solari-backend.ts:85. Readable with no key: the rate, the default size, snapshot storage and a 20 GB builder disk.
pub struct SolariPricing;

impl BackendPricing for SolariPricing {
    fn rate_usd_per_hour(&self, size: Size) -> f64 {
        rate_usd_per_hour(size)
    }
    fn default_size(&self) -> Size {
        DEFAULT_SIZE
    }
    fn snapshot_storage(&self) -> SnapshotStoragePricing {
        snapshot_storage_pricing()
    }
    fn builder_disk_gb(&self) -> Option<f64> {
        Some(BUILDER_DISK_GB as f64)
    }
}

/// solari-backend.ts:95.
pub const RESUME_CAP_MS: u64 = 30_000;
/// solari-backend.ts:98.
pub const PAUSE_MS: u64 = 4 * 60_000;
/// solari-backend.ts:100.
pub const STATE_READ_MS: u64 = 30_000;
/// solari-backend.ts:103. The longest idleTimeoutMs the create API has taken.
pub const IDLE_TIMEOUT_MAX_MS: u64 = 6 * 60 * 60_000;
/// solari-backend.ts:107. The longest one exec may ask the provider for; longer runs detached.
pub const SOLARI_INLINE_MAX_MS: u64 = 26_000;
/// solari-backend.ts:111.
pub const SANDBOX_UNREACHABLE: &str = "Sandbox is not reachable";
/// solari-backend.ts:114.
pub const EXEC_FAILED: &str = "exec failed";
/// solari-backend.ts:117.
pub const NOT_PAUSABLE: &str = "Not pausable";
/// solari-backend.ts:121.
pub const SNAPSHOT_NOT_FOUND: &str = "Snapshot not found";
/// solari-backend.ts:123. How long a fork of a listed snapshot is asked again, and how often.
pub const SNAPSHOT_LISTED_MS: u64 = 3 * 60_000;
pub const SNAPSHOT_LISTED_EVERY_MS: u64 = 15_000;

/// solari-backend.ts:128. Two wake attempts, the daemon answering in 30 s, resume asked every minute for 30 minutes.
/// One value for every backend; nothing can change it.
pub fn solari_lifecycle_budgets() -> LifecycleBudgets {
    LifecycleBudgets {
        wake_attempts: JsInt::from(2),
        daemon_answers_ms: JsNumber(30_000.0),
        resume_asks: Some(LifecycleBudgetsResumeAsks { every_ms: JsNumber(60_000.0), for_ms: JsNumber(1_800_000.0) }),
    }
}

/// The lifecycle `SolariBackend::lifecycle()` answers: its budgets are `solari_lifecycle_budgets()`, no backstop, no
/// quiet read.
pub struct SolariLifecycle;

#[async_trait(?Send)]
impl Lifecycle for SolariLifecycle {
    fn budgets(&self) -> LifecycleBudgets {
        solari_lifecycle_budgets()
    }
}

/// solari-backend.ts:142. The pt_token exp claim is 60 minutes from mint.
pub const PREVIEW_TTL_MS: i64 = 60 * 60_000;

/// solari-backend.ts:146. Epoch-ms expiry of a preview token: cut at the last `.` (skipped where it is at 0 or absent),
/// then the whole string; each part base64url-decoded leniently as node does, and the first `"exp":<digits>` in it read
/// as a number. `now + PREVIEW_TTL_MS` where neither carries one.
pub fn preview_token_expiry(token: &str, now: i64) -> f64 {
    for cut in [token.rfind('.'), Some(token.len())].into_iter().flatten().filter(|&c| c > 0) {
        let decoded = String::from_utf8_lossy(&crate::node_base64::node_base64_decode(&token[..cut])).into_owned();
        if let Some(exp) = first_exp(&decoded) {
            return exp;
        }
    }
    (now + PREVIEW_TTL_MS) as f64
}

/// `/"exp":(\d+)/` on the text, read as the JS number it is.
fn first_exp(text: &str) -> Option<f64> {
    let mut rest = text;
    while let Some(at) = rest.find("\"exp\":") {
        let after = &rest[at + 6..];
        let digits = after.bytes().take_while(u8::is_ascii_digit).count();
        if digits > 0 {
            return Some(after[..digits].parse::<f64>().unwrap_or(0.0));
        }
        rest = after;
    }
    None
}

/// solari-backend.ts:66. A state word the table never learned reads running: starting, running, paused as themselves;
/// archived, releasing and gone are gone; anything else running.
pub fn state_of(state: &str) -> MachineState {
    match state {
        "starting" => MachineState::Starting,
        "paused" => MachineState::Paused,
        "archived" | "releasing" | "gone" => MachineState::Gone,
        _ => MachineState::Running,
    }
}

/// The wire word a state reads as in a warn line or a words sentence.
fn state_word(state: &MachineState) -> &'static str {
    match state {
        MachineState::Starting => "starting",
        MachineState::Running => "running",
        MachineState::Paused => "paused",
        MachineState::Gone => "gone",
    }
}

/// solari-backend.ts:161.
pub struct SolariBackend {
    api_key: String,
    base_url: String,
    fetch: Arc<dyn Fetch>,
    clock: Arc<dyn RetryClock>,
    budgets: MoveBudgets,
    warn: OnLine,
}

/// The view of a sandbox the provider answers. Every field but the id may be absent.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct SandboxView {
    sandbox_id: Option<String>,
    /// A kind the table never learned reads as none: the listing never reads it.
    #[serde(default, deserialize_with = "known_kind")]
    kind: Option<MachineKind>,
    state: Option<String>,
    metadata: Option<IndexMap<String, String>>,
    cpu: Option<JsNumber>,
    mem_mb: Option<JsNumber>,
    disk_gb: Option<JsNumber>,
    created_at: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CreateReply {
    sandbox_id: String,
    kind: Option<MachineKind>,
    stream_url: Option<String>,
    state: Option<String>,
    /// `Some(None)` is a `createdAt` the reply carries as null, which is not undefined.
    #[serde(default, deserialize_with = "carried_text")]
    created_at: Option<Option<String>>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ListPage {
    sandboxes: Option<Vec<SandboxView>>,
    next_cursor: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct SnapshotView {
    id: String,
    name: Option<String>,
    size_bytes: JsNumber,
    created_at: Option<String>,
    #[serde(default, deserialize_with = "double_option")]
    parent: Option<Option<String>>,
}

fn known_kind<'de, D: Deserializer<'de>>(d: D) -> std::result::Result<Option<MachineKind>, D::Error> {
    Ok(serde_json::from_value(Value::deserialize(d)?).ok())
}

fn known_status<'de, D: Deserializer<'de>>(d: D) -> std::result::Result<TemplateRowStatus, D::Error> {
    Ok(serde_json::from_value(Value::deserialize(d)?).unwrap_or(TemplateRowStatus::Building))
}

/// A key the reply carries as null reads `Some(None)`, a text as itself, and one it does not carry reads None.
fn carried_text<'de, D: Deserializer<'de>>(d: D) -> std::result::Result<Option<Option<String>>, D::Error> {
    Ok(Some(match Value::deserialize(d)? {
        Value::String(text) => Some(text),
        _ => None,
    }))
}

/// A key the reply carries as null reads `Some(None)`; one it does not carry reads None.
fn double_option<'de, D: Deserializer<'de>>(d: D) -> std::result::Result<Option<Option<String>>, D::Error> {
    Option::<String>::deserialize(d).map(Some)
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct TemplateView {
    template_id: String,
    name: String,
    /// A status word the table never learned reads as building, as the Box backend reads one.
    #[serde(deserialize_with = "known_status")]
    status: TemplateRowStatus,
    error: Option<String>,
    created_at: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct PortView {
    url: String,
    token: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct SnapshotIdReply {
    snapshot_id: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct PromoteReply {
    template_id: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct DownloadReply {
    url: String,
}

fn other(message: String) -> EngineError {
    EngineError::Other { message, code: None }
}

/// The JSON number a JavaScript number writes as.
fn js_num(x: f64) -> Value {
    serde_json::to_value(JsNumber(x)).unwrap_or_default()
}

/// `encodeURIComponent`.
fn encode_uri_component(text: &str) -> String {
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

/// A random lowercase v4 UUID, as `crypto.randomUUID()` writes it.
fn random_uuid() -> String {
    let mut b: [u8; 16] = rand::random();
    b[6] = (b[6] & 0x0f) | 0x40;
    b[8] = (b[8] & 0x3f) | 0x80;
    let h = hex::encode(b);
    format!("{}-{}-{}-{}-{}", &h[0..8], &h[8..12], &h[12..16], &h[16..20], &h[20..32])
}

/// The error text a refusal's `error` field reads as: JavaScript's String of it where it is truthy, "" where it is not.
fn error_text(v: Option<&Value>) -> String {
    match v {
        Some(Value::Number(n)) if n.as_f64() == Some(0.0) => String::new(),
        Some(Value::Null | Value::Bool(false)) | None => String::new(),
        Some(v) => js_string(v),
    }
}

fn template_row_of(t: TemplateView) -> TemplateRow {
    TemplateRow { id: t.template_id, name: t.name, status: t.status, error: t.error, created_at: t.created_at }
}

impl SolariBackend {
    /// solari-backend.ts:191.
    pub fn new(opts: SolariBackendOptions) -> Arc<Self> {
        let default_warn: OnLine = Arc::new(|line: &str| eprintln!("{line}"));
        let default_clock: Arc<dyn RetryClock> = Arc::new(RealRetryClock);
        Arc::new(SolariBackend {
            api_key: opts.api_key,
            base_url: opts.base_url.unwrap_or_else(|| "https://api.getsolari.com".to_string()),
            fetch: opts.fetch.unwrap_or_else(platform_fetch),
            clock: opts.clock.unwrap_or(default_clock),
            budgets: MoveBudgets {
                pause_ms: opts.budgets.pause_ms.unwrap_or(PAUSE_MS as f64),
                resume_cap_ms: opts.budgets.resume_cap_ms.unwrap_or(RESUME_CAP_MS as f64),
                state_read_ms: opts.budgets.state_read_ms.unwrap_or(STATE_READ_MS as f64),
                read_ms: opts.budgets.read_ms.unwrap_or(PROVIDER_READ_CAP_MS as f64),
            },
            warn: opts.warn.unwrap_or(default_warn),
        })
    }

    /// The budgets after defaults: pause `PAUSE_MS`, resume `RESUME_CAP_MS`, read of state `STATE_READ_MS`, plain read
    /// `PROVIDER_READ_CAP_MS`, each replaced where given.
    pub fn budgets(&self) -> MoveBudgets {
        self.budgets
    }

    pub fn clock(&self) -> Arc<dyn RetryClock> {
        self.clock.clone()
    }

    /// A handle to this backend for a machine it makes. The backend holds nothing that changes, so a copy behaves as this one.
    fn shared(&self) -> Arc<SolariBackend> {
        Arc::new(SolariBackend {
            api_key: self.api_key.clone(),
            base_url: self.base_url.clone(),
            fetch: self.fetch.clone(),
            clock: self.clock.clone(),
            budgets: self.budgets,
            warn: self.warn.clone(),
        })
    }

    /// solari-backend.ts:201. One call to the provider; the answer's JSON (`{}` for an empty body). `cap_ms` cuts the call
    /// off (a GET with none takes `read_ms`), `signal` is the caller's stop; both ride the one fetch. Retries and
    /// failures in spec/solari_call.md.
    pub async fn request<T: DeserializeOwned>(
        &self,
        method: &str,
        path: &str,
        body: Option<Value>,
        cap_ms: Option<f64>,
        signal: Option<&Cancel>,
    ) -> Result<T> {
        let (value, _) = self.call(method, path, body, None, cap_ms, signal).await?;
        serde_json::from_value(value).map_err(|e| other(e.to_string()))
    }

    /// solari-backend.ts:207. Keyed: a fetch that throws is sent once more under the key. Returns the answer's JSON and
    /// the reply it came on.
    async fn call(
        &self,
        method: &str,
        path: &str,
        body: Option<Value>,
        key: Option<&str>,
        cap_ms: Option<f64>,
        signal: Option<&Cancel>,
    ) -> Result<(Value, FetchResponse)> {
        let mut headers = vec![("Authorization".to_string(), format!("Bearer {}", self.api_key))];
        if let Some(key) = key {
            headers.push(("Idempotency-Key".to_string(), key.to_string()));
        }
        if body.is_some() {
            headers.push(("Content-Type".to_string(), "application/json".to_string()));
        }
        let payload = body.as_ref().map(|b| b.to_string().into_bytes());
        let cap = cap_ms.or_else(|| (method == "GET").then_some(self.budgets.read_ms));
        let mut resent = false;
        let mut road_retries: u32 = 0;
        let mut attempt: u32 = 1;
        loop {
            let req = FetchRequest {
                method: method.to_string(),
                url: format!("{}{path}", self.base_url),
                headers: headers.clone(),
                body: payload.clone(),
                abort: abort(cap, signal),
            };
            let res = match self.fetch.fetch(req).await {
                Ok(res) => res,
                Err(road) => {
                    let e = EngineError::Road(road);
                    if let Some(code) = road_code(&e) {
                        road_retries += 1;
                        if road_retries >= ROAD_TRIES {
                            return Err(e);
                        }
                        let line = provider_road_retry_line(
                            &format!("{method} {path}"),
                            &code,
                            f64::from(road_retries + 1),
                            f64::from(ROAD_TRIES),
                        );
                        (self.warn)(&line);
                        self.clock.sleep(road_backoff_ms(road_retries, self.clock.random())).await;
                        continue;
                    }
                    if key.is_none() || resent {
                        return Err(e);
                    }
                    resent = true;
                    self.clock.sleep(backoff_ms(attempt, self.clock.random())).await;
                    continue;
                }
            };
            if res.ok() {
                let text = res.text();
                let value =
                    if text.is_empty() { Value::Object(Map::new()) } else { parse_json(&text).map_err(|e| other(e.to_string()))? };
                return Ok((value, res));
            }
            let raw = js_trim(&res.text()).to_string();
            let parsed = match parse_json(&raw) {
                Ok(v) => v,
                Err(_) if raw.is_empty() => json!({}),
                Err(_) => json!({ "error": utf16::cut(&raw, 300) }),
            };
            if parsed.is_null() {
                return Err(other("Cannot read properties of null (reading 'code')".to_string()));
            }
            let code = parsed.get("code").and_then(Value::as_str);
            let message = error_text(parsed.get("error"));
            let request_id = res.header(REQUEST_ID_HEADER);
            let mut e: WspError = classify(res.status, code, Some(message.as_str()), request_id);
            if !should_retry(&e, attempt) {
                if e.message.is_empty() {
                    let kind = serde_json::to_value(e.kind).ok().and_then(|k| k.as_str().map(str::to_string)).unwrap_or_default();
                    e.message = format!("{kind} ({})", e.status);
                }
                return Err(EngineError::Provider(e));
            }
            self.clock.sleep(backoff_ms(attempt, self.clock.random())).await;
            attempt += 1;
        }
    }

    /// solari-backend.ts:256. A fork the provider answers "Snapshot not found" is read against its listing: a snapshot
    /// the listing holds is not gone, so the fork is asked again under a key of its own inside a bound, and only an
    /// unlisted one is missing.
    async fn create_machine(&self, spec: MachineSpec, waiting: Option<Waiting>) -> Result<Arc<dyn Machine>> {
        let started = self.clock.now();
        let mut ask = 1;
        loop {
            let key = match (ask, &spec.idempotency_key) {
                (1, Some(key)) => key.clone(),
                _ => random_uuid(),
            };
            let e = match self.create_once(&spec, &key).await {
                Ok(machine) => return Ok(machine),
                Err(e) => e,
            };
            let Some(snapshot) = spec.from_snapshot.as_deref() else { return Err(e) };
            if !is_missing(&e) || e.to_string() != SNAPSHOT_NOT_FOUND {
                return Err(e);
            }
            let listed = <Self as MachineBackend>::list_snapshots(self, None).await.is_ok_and(|rows| rows.iter().any(|r| r.id == snapshot));
            if !listed {
                return Err(e);
            }
            let waited = self.clock.now() - started;
            if waited as f64 + SNAPSHOT_LISTED_EVERY_MS as f64 > SNAPSHOT_LISTED_MS as f64 {
                let message = snapshot_listed_refused_line(snapshot, waited as f64);
                return Err(match e {
                    EngineError::Provider(w) => EngineError::Provider(WspError { kind: ErrorKind::Transient, message, ..w }),
                    other => other,
                });
            }
            (self.warn)(&snapshot_listed_wait_line(snapshot, SNAPSHOT_LISTED_EVERY_MS as f64, waited as f64));
            if let Some(waiting) = &waiting {
                waiting(&image_served_wait_line(&provider_key_name("solari"), SNAPSHOT_LISTED_EVERY_MS as f64), None);
            }
            self.clock.sleep(SNAPSHOT_LISTED_EVERY_MS).await;
            ask += 1;
        }
    }

    /// solari-backend.ts:278. Only /sandboxes honours the key (measured 2026-09-04); one key rides every retry of this call.
    async fn create_once(&self, spec: &MachineSpec, key: &str) -> Result<Arc<dyn Machine>> {
        let mut body = Map::new();
        body.insert("kind".into(), serde_json::to_value(spec.kind).unwrap_or_default());
        if let Some(template) = spec.template.as_ref().filter(|t| !t.is_empty()) {
            body.insert("template".into(), json!(template));
        }
        if let Some(from) = spec.from_snapshot.as_ref().filter(|s| !s.is_empty()) {
            body.insert("fromSnapshot".into(), json!(from));
        }
        if let Some(cpu) = spec.cpu.filter(|c| c.0 != 0.0) {
            body.insert("cpu".into(), js_num(cpu.0));
        }
        if let Some(mem) = spec.mem_mb.filter(|m| m.0 != 0.0) {
            body.insert("memMb".into(), js_num(mem.0));
        }
        if let Some(disk) = spec.disk_gb.filter(|d| d.0 != 0.0) {
            body.insert("diskGb".into(), js_num(disk.0));
        }
        if let Some(envs) = &spec.envs {
            body.insert("envs".into(), serde_json::to_value(envs).unwrap_or_default());
        }
        if let Some(labels) = &spec.labels {
            body.insert("metadata".into(), serde_json::to_value(labels).unwrap_or_default());
        }
        if let Some(on_idle) = spec.on_idle {
            body.insert("lifecycle".into(), json!({ "onTimeout": serde_json::to_value(on_idle).unwrap_or_default() }));
        }
        if let Some(idle) = spec.idle_timeout_ms.filter(|i| i.0 != 0.0) {
            body.insert("timeoutMs".into(), js_num(idle.0.min(IDLE_TIMEOUT_MAX_MS as f64)));
        }
        let (value, reply) = self.call("POST", "/sandboxes", Some(Value::Object(body)), Some(key), None, None).await?;
        let res: CreateReply = serde_json::from_value(value).map_err(|e| other(e.to_string()))?;
        let seen = res.created_at.map(|created_at| Seen {
            state: state_of(res.state.as_deref().unwrap_or("running")),
            created_at,
            paused_at: None,
        });
        let replayed = reply.header("Idempotent-Replayed") == Some("true");
        Ok(Arc::new(SolariMachine {
            backend: self.shared(),
            id: res.sandbox_id,
            kind: res.kind.unwrap_or(spec.kind),
            stream_url: res.stream_url,
            labels: spec.labels.clone(),
            seen,
            replayed,
        }))
    }
}

#[async_trait(?Send)]
impl MachineBackend for SolariBackend {
    /// solari-backend.ts:162. Every field in spec/solari_call.md; sizes are `[DEFAULT_SIZE]` with its rate.
    fn capabilities(&self) -> Capabilities {
        Capabilities {
            live_clone_forks: true,
            pause_mode: Some(PauseMode::Memory),
            replaces_machine: true,
            preview_urls: true,
            signed_urls: true,
            callback_relay: true,
            disk_snapshots: true,
            images: true,
            snapshots_any_life: false,
            snapshot_listing: true,
            templates: true,
            kept: false,
            copies: true,
            own_network: true,
            sizes: vec![MachineSizeOffer {
                cpu: JsNumber(DEFAULT_SIZE.cpu),
                mem_mb: JsNumber(DEFAULT_SIZE.mem_mb),
                rate_usd_per_hour: JsNumber(rate_usd_per_hour(DEFAULT_SIZE)),
                name: None,
            }],
            sizes_last: true,
        }
    }
    fn pricing(&self) -> Arc<dyn BackendPricing> {
        Arc::new(SolariPricing)
    }
    fn lifecycle(&self) -> Option<Arc<dyn Lifecycle>> {
        Some(Arc::new(SolariLifecycle))
    }
    /// check_key, list_snapshots and the four template methods.
    fn offers(&self) -> BackendOffers {
        BackendOffers { check_key: true, list_snapshots: true, templates: true, ..BackendOffers::default() }
    }
    /// solari-backend.ts:256. `waiting` hears each wait as `image_served_wait_line("Solari", SNAPSHOT_LISTED_EVERY_MS)`,
    /// with no provider box. `sent` is not read.
    async fn create(&self, spec: MachineSpec, waiting: Option<Waiting>, _sent: Option<String>) -> Result<Arc<dyn Machine>> {
        self.create_machine(spec, waiting).await
    }
    /// solari-backend.ts:309.
    async fn get(&self, id: &str) -> Result<Arc<dyn Machine>> {
        let view: SandboxView = self.request("GET", &format!("/sandboxes/{}", encode_uri_component(id)), None, None, None).await?;
        Ok(Arc::new(SolariMachine {
            backend: self.shared(),
            id: view.sandbox_id.unwrap_or_else(|| id.to_string()),
            kind: view.kind.unwrap_or(MachineKind::Sandbox),
            stream_url: None,
            labels: view.metadata,
            seen: Some(Seen { state: state_of(view.state.as_deref().unwrap_or("")), created_at: view.created_at, paused_at: None }),
            replayed: false,
        }))
    }
    /// solari-backend.ts:317.
    async fn list(&self, labels: Option<&IndexMap<String, String>>) -> Result<Vec<MachineListRow>> {
        let mut out = Vec::new();
        let mut cursor: Option<String> = None;
        loop {
            let mut params = url::form_urlencoded::Serializer::new(String::new());
            for (k, v) in labels.into_iter().flatten() {
                params.append_pair(&format!("metadata.{k}"), v);
            }
            if let Some(c) = cursor.as_deref().filter(|c| !c.is_empty()) {
                params.append_pair("cursor", c);
            }
            let qs = params.finish();
            let path = if qs.is_empty() { "/sandboxes".to_string() } else { format!("/sandboxes?{qs}") };
            let page: ListPage = self.request("GET", &path, None, None, None).await?;
            for s in page.sandboxes.unwrap_or_default() {
                out.push(MachineListRow {
                    id: s.sandbox_id.unwrap_or_default(),
                    state: state_of(s.state.as_deref().unwrap_or("")),
                    labels: s.metadata.unwrap_or_default(),
                    size: match (s.cpu, s.mem_mb) {
                        (Some(cpu), Some(mem_mb)) => Some(WorkspaceSize { cpu, mem_mb }),
                        _ => None,
                    },
                });
            }
            cursor = page.next_cursor.filter(|c| !c.is_empty());
            if cursor.is_none() {
                return Ok(out);
            }
        }
    }
    /// solari-backend.ts:341.
    async fn delete_snapshot(&self, id: &str) -> Result<()> {
        self.request::<Value>("DELETE", &format!("/snapshots/{}", encode_uri_component(id)), None, None, None).await.map(drop)
    }
    /// solari-backend.ts:305. Lists the templates; answers `Value::Null` (the TS `void`).
    async fn check_key(&self) -> Result<Value> {
        self.list_templates().await?;
        Ok(Value::Null)
    }
    /// solari-backend.ts:364. `ids` is not read: the provider's whole listing.
    async fn list_snapshots(&self, _ids: Option<&[String]>) -> Result<Vec<SnapshotRow>> {
        let page: Value = self.request("GET", "/snapshots", None, None, None).await?;
        let Some(rows) = page.get("snapshots").and_then(Value::as_array) else {
            return Err(other("GET /snapshots answered without a snapshots array".to_string()));
        };
        let rows: Vec<SnapshotView> = serde_json::from_value(Value::Array(rows.clone())).map_err(|e| other(e.to_string()))?;
        Ok(rows
            .into_iter()
            .map(|s| SnapshotRow {
                id: s.id,
                name: s.name,
                size_bytes: s.size_bytes,
                restored_bytes: Some(s.size_bytes),
                created_at: s.created_at,
                parent: s.parent,
            })
            .collect())
    }
    /// solari-backend.ts:345.
    async fn promote_snapshot(&self, snapshot_id: &str, name: &str) -> Result<String> {
        let res: PromoteReply = self
            .request(
                "POST",
                &format!("/snapshots/{}/promote", encode_uri_component(snapshot_id)),
                Some(json!({ "name": name })),
                None,
                None,
            )
            .await?;
        Ok(res.template_id)
    }
    /// solari-backend.ts:350.
    async fn get_template(&self, id: &str) -> Result<TemplateRow> {
        let view: TemplateView = self.request("GET", &format!("/templates/{}", encode_uri_component(id)), None, None, None).await?;
        Ok(template_row_of(view))
    }
    /// solari-backend.ts:354.
    async fn list_templates(&self) -> Result<Vec<TemplateRow>> {
        let page: Value = self.request("GET", "/templates", None, None, None).await?;
        let Some(rows) = page.get("templates").and_then(Value::as_array) else {
            return Err(other("GET /templates answered without a templates array".to_string()));
        };
        let rows: Vec<TemplateView> = serde_json::from_value(Value::Array(rows.clone())).map_err(|e| other(e.to_string()))?;
        Ok(rows.into_iter().map(template_row_of).collect())
    }
    /// solari-backend.ts:360.
    async fn delete_template(&self, id: &str) -> Result<()> {
        self.request::<Value>("DELETE", &format!("/templates/{}", encode_uri_component(id)), None, None, None).await.map(drop)
    }
}

/// solari-backend.ts:382. A machine handle: its id, kind, stream URL, labels, what the provider said when it was made,
/// and whether a create was a replay.
pub struct SolariMachine {
    backend: Arc<SolariBackend>,
    id: String,
    kind: MachineKind,
    stream_url: Option<String>,
    labels: Option<IndexMap<String, String>>,
    seen: Option<Seen>,
    replayed: bool,
}

impl SolariMachine {
    /// `/sandboxes/<encodeURIComponent id><suffix>`.
    fn route(&self, suffix: &str) -> String {
        format!("/sandboxes/{}{suffix}", encode_uri_component(&self.id))
    }

    /// The provider's word on the machine under `cap_ms`; `None` where the read could not be had, `gone` on a 404.
    async fn read_state(&self, cap_ms: f64) -> Option<MachineState> {
        match self.backend.request::<SandboxView>("GET", &self.route(""), None, Some(cap_ms), None).await {
            Ok(view) => Some(state_of(view.state.as_deref().unwrap_or(""))),
            Err(e) if is_missing(&e) => Some(MachineState::Gone),
            Err(_) => None,
        }
    }
}

#[async_trait(?Send)]
impl Machine for SolariMachine {
    fn id(&self) -> &str {
        &self.id
    }
    fn kind(&self) -> MachineKind {
        self.kind
    }
    fn stream_url(&self) -> Option<&str> {
        self.stream_url.as_deref()
    }
    fn labels(&self) -> Option<&IndexMap<String, String>> {
        self.labels.as_ref()
    }
    fn seen(&self) -> Option<&Seen> {
        self.seen.as_ref()
    }
    fn replayed(&self) -> bool {
        self.replayed
    }
    /// preview_url, describe and metrics.
    fn offers(&self) -> MachineOffers {
        MachineOffers { preview_url: true, describe: true, metrics: true, ..MachineOffers::default() }
    }
    /// solari-backend.ts:397.
    async fn exec(&self, cmd: &str, opts: ExecOptions) -> Result<ExecResult> {
        let timeout_ms = opts.timeout_ms.unwrap_or(INLINE_EXEC_MS);
        if timeout_ms > SOLARI_INLINE_MAX_MS {
            return exec_detached(self, cmd, RunOptions { deadline_ms: timeout_ms, ..RunOptions::default() }).await;
        }
        let body = json!({ "cmd": "bash", "args": ["-c", format!("{EXEC_ENV}\n{cmd}")], "timeoutMs": timeout_ms });
        match self.backend.request::<ExecResult>("POST", &self.route("/exec"), Some(body), None, None).await {
            Ok(res) => Ok(res),
            Err(EngineError::Provider(w)) if w.message == SANDBOX_UNREACHABLE => {
                Err(EngineError::machine_unreachable(&self.id, &w.message, w.status, &machine_unreachable_line(&w.message)))
            }
            Err(EngineError::Provider(w)) if w.status == 502 && w.message == EXEC_FAILED => {
                Err(EngineError::exec_failed(&self.id, &w.message, 502))
            }
            Err(e) => Err(e),
        }
    }
    /// solari-backend.ts:417. exec-detached's `exec_detached(self, script, opts)`.
    async fn run(&self, script: &str, opts: RunOptions) -> Result<ExecResult> {
        exec_detached(self, script, opts).await
    }
    /// solari-backend.ts:423. `opts` is not read.
    async fn snapshot(&self, name: &str, life: MachineLife, _opts: SnapshotOptions) -> Result<String> {
        if !life.first_life {
            return Err(EngineError::NotFirstLife { machine_id: self.id.clone(), action: format!("snapshot {name}") });
        }
        let res: SnapshotIdReply =
            self.backend.request("POST", &self.route("/snapshots"), Some(json!({ "name": name })), None, None).await?;
        Ok(res.snapshot_id)
    }
    /// solari-backend.ts:434. `wait_ms` is not read: the budget is the backend's.
    async fn pause(&self, _wait_ms: Option<u64>) -> Result<()> {
        let MoveBudgets { pause_ms, state_read_ms, .. } = self.backend.budgets();
        let clock = self.backend.clock();
        let started = clock.now() as f64;
        let deadline = started + pause_ms;
        let mut unanswered: Option<EngineError> = None;
        let mut attempt = 1;
        loop {
            let left = deadline - clock.now() as f64;
            if left > 0.0 {
                let cap = if attempt == 1 { left / 2.0 } else { left };
                match self.backend.request::<Value>("POST", &self.route("/pause"), Some(json!({})), Some(cap), None).await {
                    Ok(_) => return Ok(()),
                    Err(e) => {
                        if let EngineError::Provider(w) = &e {
                            if w.status == 409 && w.message == NOT_PAUSABLE {
                                return Err(EngineError::nap_refused(&self.id, &w.message));
                            }
                        }
                        if !is_capped(&e) && !is_network_error(&e) {
                            return Err(e);
                        }
                        unanswered = Some(e);
                    }
                }
            }
            let reads = self.read_state(state_read_ms).await;
            if reads == Some(MachineState::Paused) {
                return Ok(());
            }
            if attempt == 1 && reads == Some(MachineState::Running) && deadline > clock.now() as f64 {
                attempt += 1;
                continue;
            }
            let words = move_timed_out_line(Move::Pause, clock.now() as f64 - started, reads.as_ref().map(state_word));
            (self.backend.warn)(&format!("{}: {words}", self.id));
            return Err(EngineError::MoveUnanswered { message: words, cause: unanswered.map(Box::new) });
        }
    }
    /// solari-backend.ts:467.
    async fn resume(&self, signal: Option<&Cancel>) -> Result<()> {
        let MoveBudgets { resume_cap_ms, state_read_ms, .. } = self.backend.budgets();
        match self.backend.request::<Value>("POST", &self.route("/resume"), Some(json!({})), Some(resume_cap_ms), signal).await {
            Ok(_) => Ok(()),
            Err(e) => {
                if !is_capped(&e) && !is_network_error(&e) {
                    return Err(e);
                }
                let reads = self.read_state(state_read_ms).await;
                if matches!(reads, Some(MachineState::Running | MachineState::Starting)) {
                    return Ok(());
                }
                let word = reads.as_ref().map_or("nothing", state_word);
                (self.backend.warn)(&format!("{}: {RESUME_UNANSWERED}, which still reads {word}", self.id));
                Err(EngineError::ResumeUnanswered { message: RESUME_UNANSWERED.to_string(), cause: Some(Box::new(e)) })
            }
        }
    }
    /// solari-backend.ts:490.
    async fn kill(&self) -> Result<()> {
        self.backend.request::<Value>("DELETE", &self.route(""), None, None, None).await.map(drop)
    }
    /// solari-backend.ts:494.
    async fn state(&self) -> Result<MachineState> {
        match self.backend.request::<SandboxView>("GET", &self.route(""), None, None, None).await {
            Ok(view) => Ok(state_of(view.state.as_deref().unwrap_or(""))),
            Err(e) if is_missing(&e) => Ok(MachineState::Gone),
            Err(e) => Err(e),
        }
    }
    /// solari-backend.ts:525.
    async fn download_url(&self, path: &str) -> Result<String> {
        let res: DownloadReply = self
            .backend
            .request("GET", &self.route(&format!("/files/download-url?path={}", encode_uri_component(path))), None, None, None)
            .await?;
        Ok(res.url)
    }
    /// solari-backend.ts:532.
    async fn upload_url(&self, path: &str) -> Result<String> {
        let res: DownloadReply = self
            .backend
            .request("GET", &self.route(&format!("/files/upload-url?path={}", encode_uri_component(path))), None, None, None)
            .await?;
        Ok(res.url)
    }
    /// solari-backend.ts:518.
    async fn preview_url(&self, port: u16) -> Result<PreviewReach> {
        let res: PortView = self.backend.request("GET", &self.route(&format!("/ports/{port}")), None, None, None).await?;
        let expires_at = preview_token_expiry(&res.token, RealRetryClock.now());
        Ok(PreviewReach { url: res.url, token: res.token, expires_at: JsNumber(expires_at) })
    }
    /// solari-backend.ts:504.
    async fn describe(&self) -> Result<MachineShape> {
        let view: SandboxView = self.backend.request("GET", &self.route(""), None, None, None).await?;
        Ok(MachineShape { cpu: view.cpu, mem_mb: view.mem_mb, disk_gb: view.disk_gb, created_at: view.created_at, used_bytes: None })
    }
    /// solari-backend.ts:514.
    async fn metrics(&self) -> Result<()> {
        self.backend.request::<Value>("GET", &self.route("/metrics"), None, None, None).await.map(drop)
    }
}
