// SPDX-License-Identifier: AGPL-3.0-only
//! The JSON-RPC lines wsp writes to `codex app-server` and the reader of what it prints. TS adapter-codex/src/rpc.ts.
//! Spec: spec/codex_rpc.md. Every line is one JSON object, keys in the order the TS literal writes them.

use indexmap::IndexMap;
use serde_json::{Map, Number, Value};

use super::command::AccessParams;

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
        todo!()
    }
}

/// `{"method":"initialized"}`. TS rpc.ts:27.
pub const INITIALIZED_LINE: &str = r#"{"method":"initialized"}"#;

/// `{"id":"wsp-account","method":"account/read","params":{}}`. TS rpc.ts:30.
pub const ACCOUNT_READ_LINE: &str = r#"{"id":"wsp-account","method":"account/read","params":{}}"#;

/// TS rpc.ts:23.
pub fn initialize_line() -> String {
    todo!()
}

/// `details` asks for each banked reset in full. TS rpc.ts:34.
pub fn rate_limits_read_line(details: bool) -> String {
    todo!()
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
    todo!()
}

/// TS rpc.ts:56.
pub fn thread_resume_line(o: &ThreadOptions, thread_id: &str) -> String {
    todo!()
}

/// The side question's ephemeral read-only fork. TS rpc.ts:63.
pub fn thread_fork_line(thread_id: &str, cwd: Option<&str>, model: Option<&str>, developer_instructions: &str) -> String {
    todo!()
}

/// A copy written to disk as a thread of its own, through `last_turn_id` where given. TS rpc.ts:75.
pub fn thread_copy_line(o: &ThreadOptions, thread_id: &str, last_turn_id: Option<&str>) -> String {
    todo!()
}

/// TS rpc.ts:82.
pub fn thread_unsubscribe_line(thread_id: &str) -> String {
    todo!()
}

/// TS rpc.ts:88.
pub fn thread_revert_line(thread_id: &str, before_turn_id: &str) -> String {
    todo!()
}

/// One page of the thread's turns, newest first; `cursor` is written `null` where none. TS rpc.ts:93.
pub fn thread_turns_list_line(thread_id: &str, cursor: Option<&str>, limit: u64) -> String {
    todo!()
}

/// TS rpc.ts:99.
pub fn turn_start_line(thread_id: &str, text: &str, images: &[String], effort: Option<&str>) -> String {
    todo!()
}

/// TS rpc.ts:107.
pub fn thread_compact_start_line(thread_id: &str) -> String {
    todo!()
}

/// TS rpc.ts:111.
pub fn turn_steer_line(id: &str, thread_id: &str, turn_id: &str, text: &str) -> String {
    todo!()
}

/// TS rpc.ts:116.
pub fn turn_interrupt_line(id: &str, thread_id: &str, turn_id: &str) -> String {
    todo!()
}

/// The only two decisions wsp sends. TS rpc.ts:122.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Decision {
    Accept,
    Decline,
}

/// TS rpc.ts:122.
pub fn decision_line(id: &RequestId, decision: Decision) -> String {
    todo!()
}

/// TS rpc.ts:127.
pub fn refuse_request_line(id: &RequestId, method: &str) -> String {
    todo!()
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
    todo!()
}
