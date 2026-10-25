// SPDX-License-Identifier: AGPL-3.0-only
//! box-backend.ts:347-786 at a4033dc34. Spec: spec/box_request.md.
use std::collections::{BTreeSet, HashMap};
use std::sync::{Arc, Mutex};

use serde::Deserialize;
use serde_json::Value;
use tokio_util::sync::CancellationToken;

use super::plan::BoxLimits;
use crate::errors::{EngineError, Result, RetryClock, WspError};
use crate::fetch::Fetch;
use crate::machine::{CloudLimits, TemplateRow};

/// box-backend.ts:349. A box as the API answers it; every field but `id` and `state` may be absent or null.
#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BoxView {
    pub id: String,
    #[serde(default)]
    pub name: Option<String>,
    pub state: String,
    #[serde(default)]
    pub error: Option<String>,
    #[serde(default)]
    pub last_snapshot_status: Option<String>,
    #[serde(default)]
    pub last_snapshot_attempt_at: Option<String>,
    #[serde(default)]
    pub snapshot_completed_at: Option<String>,
    #[serde(default, rename = "type")]
    pub kind: Option<String>,
    #[serde(default)]
    pub vcpu: Option<f64>,
    #[serde(default, rename = "memoryGB")]
    pub memory_gb: Option<f64>,
    #[serde(default)]
    pub created_at: Option<String>,
    #[serde(default)]
    pub archive_after: Option<String>,
    #[serde(default)]
    pub subdomain: Option<String>,
}

/// box-backend.ts:368.
#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CommandView {
    #[serde(default)]
    pub exit_code: Option<i64>,
    #[serde(default)]
    pub stdout: Option<String>,
    #[serde(default)]
    pub stderr: Option<String>,
    #[serde(default)]
    pub timed_out: Option<bool>,
}

/// box-backend.ts:375.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NamedSnapshotView {
    pub name: String,
    /// "saving", "ready" or "failed".
    pub status: String,
    #[serde(default)]
    pub error: Option<String>,
    #[serde(default)]
    pub source_sandbox_id: Option<String>,
    #[serde(default)]
    pub snapshot_id: Option<String>,
    #[serde(default, rename = "type")]
    pub kind: Option<String>,
    #[serde(default)]
    pub size_bytes: Option<f64>,
    pub created_at: String,
}

/// box-backend.ts:386.
#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SnapshotView {
    pub id: String,
    #[serde(default)]
    pub sandbox_id: Option<String>,
    #[serde(default)]
    pub status: Option<String>,
    #[serde(default)]
    pub kind: Option<String>,
    /// `Some(None)` is a chainId the API answered as null, which the listing passes on as `parent: null`.
    #[serde(default, deserialize_with = "crate::box_backend::request::double_option")]
    pub chain_id: Option<Option<String>>,
    #[serde(default)]
    pub created_at: Option<String>,
    #[serde(default)]
    pub completed_at: Option<String>,
    #[serde(default)]
    pub size_bytes: Option<f64>,
    #[serde(default)]
    pub content_size_bytes: Option<f64>,
}

/// Reads a present null as `Some(None)` and an absent field as `None`.
pub fn double_option<'de, D: serde::Deserializer<'de>>(d: D) -> std::result::Result<Option<Option<String>>, D::Error> {
    Option::<String>::deserialize(d).map(Some)
}

/// box-backend.ts:409.
pub fn stop_refusal_of(view: &BoxView) -> String {
    todo!()
}

/// box-backend.ts:415.
pub fn template_row_of(s: &NamedSnapshotView) -> TemplateRow {
    todo!()
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

/// box-backend.ts:446. `body` is the refusal's parsed JSON, `Value::Null` where it was not JSON.
pub fn request_limit_wait_ms(body: &Value) -> u64 {
    todo!()
}

/// box-backend.ts:459. A refusal read by its code.
pub fn box_refusal(e: WspError) -> WspError {
    todo!()
}

/// box-backend.ts:472. The provider saying the box is not ready for this call yet.
pub fn still_restoring(e: &EngineError) -> bool {
    todo!()
}

/// box-backend.ts:480. What a call whose answer was lost said.
pub fn road_words(e: &EngineError) -> String {
    todo!()
}

/// box-backend.ts:487. A refusal as the error it is thrown as: `EngineError::Provider` whose message is the
/// refusal's, kept even when empty (the TS's fallback words are dead).
pub fn fail(e: WspError) -> EngineError {
    todo!()
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
    pub signal: Option<CancellationToken>,
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
pub type Warn = Arc<dyn Fn(&str) + Send + Sync>;

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
    pub limits_asked: Option<futures::future::Shared<futures::future::BoxFuture<'static, std::result::Result<BoxLimits, EngineError>>>>,
    /// When each refusal (call, status, code) was last logged.
    pub refusals_logged: HashMap<String, i64>,
    pub backstops: HashMap<String, Timer>,
    pub backstops_sending: BTreeSet<String>,
    pub backstop_failures: HashMap<String, BackstopFailure>,
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
        todo!()
    }

    pub fn budgets(&self) -> BoxBudgets {
        self.shared.budgets
    }

    pub fn clock(&self) -> Arc<dyn RetryClock> {
        self.shared.clock.clone()
    }

    /// box-backend.ts:603. The answer's JSON, `{}` for an empty 2xx body.
    pub async fn request(&self, method: &str, path: &str, opts: RequestOpts) -> Result<Value> {
        todo!()
    }

    /// box-backend.ts:653.
    pub fn log_refusal(&self, call: &str, status: u16, code: Option<&str>, request_id: Option<&str>, said: &str) {
        todo!()
    }

    /// box-backend.ts:664.
    pub async fn limits(&self) -> Result<BoxLimits> {
        todo!()
    }

    /// box-backend.ts:671. `None` where the read failed.
    pub async fn place_limits(&self) -> Option<CloudLimits> {
        todo!()
    }

    /// box-backend.ts:677.
    pub async fn read_limits(&self) -> Result<BoxLimits> {
        todo!()
    }

    /// box-backend.ts:692.
    pub async fn before_start(&self) -> Result<BoxLimits> {
        todo!()
    }

    /// box-backend.ts:702.
    pub fn counted(&self) {
        todo!()
    }

    /// box-backend.ts:709. The error a refused start is thrown as: `e` itself, or the start window's line.
    pub async fn start_refused(&self, e: EngineError, what: Option<&str>) -> EngineError {
        todo!()
    }

    /// box-backend.ts:726. `Some(7200)` on the trial, `None` once it has paid.
    pub async fn ttl_cap(&self) -> Result<Option<u64>> {
        todo!()
    }

    /// box-backend.ts:733. `instant` in ms since the epoch on the backend's clock.
    pub async fn ttl_seconds_for(&self, instant: Option<i64>) -> Result<Option<u64>> {
        todo!()
    }

    /// box-backend.ts:742.
    pub fn last_timer(&self, id: &str) -> Option<Timer> {
        todo!()
    }

    /// box-backend.ts:751.
    pub async fn view(&self, id: &str) -> Result<BoxView> {
        todo!()
    }

    /// box-backend.ts:757. Reads the box until `done`, at the poll pace, inside `budget_ms`.
    pub async fn settle(
        &self,
        id: &str,
        done: &(dyn Fn(&BoxView) -> bool + Send + Sync),
        budget_ms: u64,
        what: &str,
        signal: Option<&CancellationToken>,
        name: Option<&str>,
        seen: Option<&(dyn Fn(&BoxView) + Send + Sync)>,
    ) -> Result<BoxView> {
        todo!()
    }

    /// box-backend.ts:774. Asks `call` again while the box is still restoring, inside the restore budget.
    pub async fn while_restoring<T, F, Fut>(&self, call: F, signal: Option<&CancellationToken>) -> Result<T>
    where
        F: FnMut() -> Fut + Send,
        Fut: std::future::Future<Output = Result<T>> + Send,
        T: Send,
    {
        todo!()
    }
}
