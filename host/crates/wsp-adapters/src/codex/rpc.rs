// SPDX-License-Identifier: AGPL-3.0-only
//! The JSON-RPC lines wsp writes to `codex app-server` and the reader of what it prints. TS adapter-codex/src/rpc.ts.
//! Spec: spec/codex_rpc.md. Every line is one JSON object, keys in the order the TS literal writes them.

use indexmap::IndexMap;
use serde_json::{json, Map, Number, Value};

use wsp_wire::text::json_stringify;
use wsp_words::js::{js_key_order, js_trim};
use wsp_words::v8_json::parse_json_lax;

use super::command::AccessParams;
use wsp_wire::types::JsNumber;

/// The ids of the requests a turn sends once each. TS rpc.ts:9.
pub mod request {
    pub const INITIALIZE: &str = "wsp-initialize";
    pub const THREAD: &str = "wsp-thread";
    pub const TURN: &str = "wsp-turn";
    pub const REVERT: &str = "wsp-revert";
    pub const TURNS: &str = "wsp-turns";
    pub const ACCOUNT: &str = "wsp-account";
    pub const RATE_LIMITS: &str = "wsp-rate-limits";
    pub const UNSUBSCRIBE: &str = "wsp-unsubscribe";
}

/// A request id as the server sends one, echoed back as it came. TS rpc.ts:12.
#[derive(Clone, Debug, PartialEq)]
pub enum RequestId {
    Str(String),
    Num(Number),
}

impl RequestId {
    /// `String(id)`: the string itself, or the number as JavaScript writes it (rule 4.25).
    pub fn js_string(&self) -> String {
        match self {
            RequestId::Str(s) => s.clone(),
            RequestId::Num(n) => JsNumber(n.as_f64().expect("a JSON number reads as a double")).js_string(),
        }
    }
}

/// `{"method":"initialized"}`. TS rpc.ts:27.
pub const INITIALIZED_LINE: &str = r#"{"method":"initialized"}"#;

/// `{"id":"wsp-account","method":"account/read","params":{}}`. TS rpc.ts:30.
pub const ACCOUNT_READ_LINE: &str = r#"{"id":"wsp-account","method":"account/read","params":{}}"#;

/// TS rpc.ts:23.
pub fn initialize_line() -> String {
    envelope(
        request::INITIALIZE,
        "initialize",
        json!({"clientInfo": {"name": "wsp", "title": "wsp", "version": "1"}}).as_object().unwrap().clone(),
    )
}

/// `details` asks for each banked reset in full. TS rpc.ts:34.
pub fn rate_limits_read_line(details: bool) -> String {
    let params = if details { Map::new() } else { json!({"excludeResetCreditDetails": true}).as_object().unwrap().clone() };
    envelope(request::RATE_LIMITS, "account/rateLimits/read", params)
}

/// The faster output tier, on a model the catalog marks as offering it. TS rpc.ts:42.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ServiceTier {
    Fast,
}

/// TS rpc.ts:38.
#[derive(Clone, Debug, PartialEq)]
pub struct ThreadOptions {
    pub cwd: Option<String>,
    pub model: Option<String>,
    pub service_tier: Option<ServiceTier>,
    pub access: AccessParams,
    /// Dotted config keys laid over the server's config for this thread alone; written only when non-empty.
    pub config: Option<IndexMap<String, String>>,
}

/// TS rpc.ts:50.
pub fn thread_start_line(o: &ThreadOptions) -> String {
    let mut params = Map::new();
    named(&mut params, o);
    access(&mut params, &o.access);
    config(&mut params, o);
    envelope(request::THREAD, "thread/start", params)
}

/// TS rpc.ts:56.
pub fn thread_resume_line(o: &ThreadOptions, thread_id: &str) -> String {
    let mut params = Map::new();
    params.insert("threadId".into(), Value::String(thread_id.into()));
    params.insert("excludeTurns".into(), Value::Bool(true));
    named(&mut params, o);
    access(&mut params, &o.access);
    config(&mut params, o);
    envelope(request::THREAD, "thread/resume", params)
}

/// The side question's ephemeral read-only fork. TS rpc.ts:63.
pub fn thread_fork_line(thread_id: &str, cwd: Option<&str>, model: Option<&str>, developer_instructions: &str) -> String {
    let mut params = Map::new();
    params.insert("threadId".into(), Value::String(thread_id.into()));
    params.insert("ephemeral".into(), Value::Bool(true));
    params.insert("excludeTurns".into(), Value::Bool(true));
    params.insert("sandbox".into(), Value::String("read-only".into()));
    params.insert("approvalPolicy".into(), Value::String("never".into()));
    put(&mut params, "cwd", cwd);
    put(&mut params, "model", model);
    params.insert("developerInstructions".into(), Value::String(developer_instructions.into()));
    envelope(request::THREAD, "thread/fork", params)
}

/// A copy written to disk as a thread of its own, through `last_turn_id` where given. TS rpc.ts:75.
pub fn thread_copy_line(o: &ThreadOptions, thread_id: &str, last_turn_id: Option<&str>) -> String {
    let mut params = Map::new();
    params.insert("threadId".into(), Value::String(thread_id.into()));
    put(&mut params, "lastTurnId", last_turn_id);
    params.insert("ephemeral".into(), Value::Bool(false));
    params.insert("excludeTurns".into(), Value::Bool(true));
    named(&mut params, o);
    access(&mut params, &o.access);
    config(&mut params, o);
    envelope(request::THREAD, "thread/fork", params)
}

/// TS rpc.ts:82.
pub fn thread_unsubscribe_line(thread_id: &str) -> String {
    envelope(request::UNSUBSCRIBE, "thread/unsubscribe", json!({"threadId": thread_id}).as_object().unwrap().clone())
}

/// TS rpc.ts:88.
pub fn thread_revert_line(thread_id: &str, before_turn_id: &str) -> String {
    envelope(request::REVERT, "thread/revert", json!({"threadId": thread_id, "beforeTurnId": before_turn_id}).as_object().unwrap().clone())
}

/// One page of the thread's turns, newest first; `cursor` is written `null` where none. TS rpc.ts:93.
pub fn thread_turns_list_line(thread_id: &str, cursor: Option<&str>, limit: u64) -> String {
    let params = json!({"threadId": thread_id, "cursor": cursor, "limit": limit, "sortDirection": "desc", "itemsView": "summary"});
    envelope(request::TURNS, "thread/turns/list", params.as_object().unwrap().clone())
}

/// TS rpc.ts:99.
pub fn turn_start_line(thread_id: &str, text: &str, images: &[String], effort: Option<&str>) -> String {
    let mut input = vec![json!({"type": "text", "text": text})];
    input.extend(images.iter().map(|path| json!({"type": "localImage", "path": path})));
    let mut params = Map::new();
    params.insert("threadId".into(), Value::String(thread_id.into()));
    params.insert("input".into(), Value::Array(input));
    put(&mut params, "effort", effort);
    envelope(request::TURN, "turn/start", params)
}

/// TS rpc.ts:107.
pub fn thread_compact_start_line(thread_id: &str) -> String {
    envelope(request::TURN, "thread/compact/start", json!({"threadId": thread_id}).as_object().unwrap().clone())
}

/// TS rpc.ts:111.
pub fn turn_steer_line(id: &str, thread_id: &str, turn_id: &str, text: &str) -> String {
    let params = json!({"threadId": thread_id, "expectedTurnId": turn_id, "input": [{"type": "text", "text": text}]});
    envelope(id, "turn/steer", params.as_object().unwrap().clone())
}

/// TS rpc.ts:116.
pub fn turn_interrupt_line(id: &str, thread_id: &str, turn_id: &str) -> String {
    envelope(id, "turn/interrupt", json!({"threadId": thread_id, "turnId": turn_id}).as_object().unwrap().clone())
}

/// The only two decisions wsp sends. TS rpc.ts:122.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Decision {
    Accept,
    Decline,
}

/// TS rpc.ts:122.
pub fn decision_line(id: &RequestId, decision: Decision) -> String {
    let word = match decision {
        Decision::Accept => "accept",
        Decision::Decline => "decline",
    };
    json_stringify(&json!({"id": id_value(id), "result": {"decision": word}}))
}

/// TS rpc.ts:127.
pub fn refuse_request_line(id: &RequestId, method: &str) -> String {
    json_stringify(&json!({"id": id_value(id), "error": {"code": -32601, "message": format!("wsp does not answer {method}")}}))
}

/// One message the server printed. TS rpc.ts:131.
#[derive(Clone, Debug, PartialEq)]
pub enum RpcMessage {
    Response { id: RequestId, result: Value },
    Error { id: RequestId, message: String },
    Notification { method: String, params: Map<String, Value>, emitted_at_ms: Option<f64> },
    Request { id: RequestId, method: String, params: Map<String, Value> },
}

/// One line of the process's output as a message, or `None`: codex's stderr shares the log. TS rpc.ts:144.
pub fn read_message(raw: &str) -> Option<RpcMessage> {
    let text = js_trim(raw);
    if !text.starts_with('{') {
        return None;
    }
    let Value::Object(message) = parse_json_lax(text)? else {
        return None;
    };
    let params = rec(message.get("params")).cloned().unwrap_or_default();
    if let Some(method) = message.get("method").and_then(Value::as_str) {
        let method = method.to_string();
        return Some(match id_of(message.get("id")) {
            Some(id) => RpcMessage::Request { id, method, params },
            None => RpcMessage::Notification { method, params, emitted_at_ms: message.get("emittedAtMs").and_then(Value::as_f64) },
        });
    }
    let id = id_of(message.get("id"))?;
    if let Some(error) = rec(message.get("error")) {
        let message = error.get("message").and_then(Value::as_str).unwrap_or("codex refused the request without saying why");
        return Some(RpcMessage::Error { id, message: message.to_string() });
    }
    message.get("result").map(|result| RpcMessage::Response { id, result: result.clone() })
}

fn id_of(value: Option<&Value>) -> Option<RequestId> {
    match value? {
        Value::String(s) => Some(RequestId::Str(s.clone())),
        Value::Number(n) => Some(RequestId::Num(n.clone())),
        _ => None,
    }
}

fn id_value(id: &RequestId) -> Value {
    match id {
        RequestId::Str(s) => Value::String(s.clone()),
        RequestId::Num(n) => Value::Number(n.clone()),
    }
}

fn rec(value: Option<&Value>) -> Option<&Map<String, Value>> {
    value?.as_object()
}

fn put(params: &mut Map<String, Value>, key: &str, value: Option<&str>) {
    if let Some(value) = value {
        params.insert(key.into(), Value::String(value.into()));
    }
}

fn named(params: &mut Map<String, Value>, o: &ThreadOptions) {
    put(params, "cwd", o.cwd.as_deref());
    put(params, "model", o.model.as_deref());
    if o.service_tier.is_some() {
        put(params, "serviceTier", Some("fast"));
    }
}

fn access(params: &mut Map<String, Value>, access: &AccessParams) {
    params.insert("sandbox".into(), Value::String(access.sandbox.word().into()));
    params.insert("approvalPolicy".into(), Value::String(access.approval_policy.word().into()));
}

fn config(params: &mut Map<String, Value>, o: &ThreadOptions) {
    let Some(entries) = o.config.as_ref().filter(|c| !c.is_empty()) else {
        return;
    };
    let ordered = js_key_order(entries.iter().map(|(k, v)| (k, v.clone())));
    let object: Map<String, Value> = ordered.into_iter().map(|(k, v)| (k.clone(), Value::String(v))).collect();
    params.insert("config".into(), Value::Object(object));
}

fn envelope(id: &str, method: &str, params: Map<String, Value>) -> String {
    json_stringify(&json!({"id": id, "method": method, "params": Value::Object(params)}))
}
