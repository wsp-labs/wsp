// SPDX-License-Identifier: AGPL-3.0-only
//! box-backend.ts:23-345 at a4033dc34. Spec: spec/box_plan.md.
use indexmap::IndexMap;
use serde_json::Value;

use crate::machine::{CloudLimits, LifecycleBudgets, MachineState, Size, SnapshotStoragePricing};
pub use wsp_words::words::units::StartWindow;

/// box-backend.ts:25.
pub const BOX_API_URL: &str = r#"https://boat.dev/api/v1"#;

/// box-backend.ts:30 `BoxClass["type"]`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum BoxClassType {
    Small,
    Default,
    Large,
    Xlarge,
}

impl BoxClassType {
    /// The provider's word: "small", "default", "large", "xlarge".
    pub fn as_str(self) -> &'static str {
        todo!()
    }
    /// The word read back; `None` for any other.
    pub fn parse(word: &str) -> Option<Self> {
        todo!()
    }
}

/// box-backend.ts:29.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BoxClass {
    pub kind: BoxClassType,
    /// The class as a person reads it.
    pub name: &'static str,
    pub cpu: f64,
    pub mem_mb: f64,
    pub disk_gb: f64,
    pub rate_usd_per_hour: f64,
}

/// box-backend.ts:39, in this order.
pub const BOX_CLASSES: [BoxClass; 4] = [
    BoxClass { kind: BoxClassType::Small, name: "small", cpu: 2.0, mem_mb: 4096.0, disk_gb: 12.0, rate_usd_per_hour: 0.018 },
    BoxClass { kind: BoxClassType::Default, name: "default", cpu: 4.0, mem_mb: 8192.0, disk_gb: 50.0, rate_usd_per_hour: 0.036 },
    BoxClass { kind: BoxClassType::Large, name: "large", cpu: 8.0, mem_mb: 16384.0, disk_gb: 125.0, rate_usd_per_hour: 0.072 },
    BoxClass { kind: BoxClassType::Xlarge, name: "extra large", cpu: 16.0, mem_mb: 32768.0, disk_gb: 251.0, rate_usd_per_hour: 0.2 },
];

/// box-backend.ts:49. The root disk a class reads, in GB; xlarge was never measured and has none.
pub fn box_root_disk_gb(kind: BoxClassType) -> Option<f64> {
    todo!()
}

/// A size as `boxClassFor` takes it: each figure may be absent.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct SizeAsk {
    pub cpu: Option<f64>,
    pub mem_mb: Option<f64>,
}

/// box-backend.ts:52.
pub fn box_class_for(size: SizeAsk) -> BoxClass {
    todo!()
}

/// box-backend.ts:58 `classOfType`.
pub fn class_of_type(kind: Option<&str>) -> Option<BoxClass> {
    todo!()
}

/// box-backend.ts:62. Free 0 GB, $0 a GB-month, billed from "", 10 named free, $1.70 each past them.
pub fn box_snapshot_storage() -> SnapshotStoragePricing {
    todo!()
}

/// box-backend.ts:64. The class's rate, default size 2 cpu and 4096 MB, [`box_snapshot_storage`], no builder disk.
pub struct BoxPricing;

impl crate::machine::BackendPricing for BoxPricing {
    fn rate_usd_per_hour(&self, size: Size) -> f64 {
        todo!()
    }
    fn default_size(&self) -> Size {
        todo!()
    }
    fn snapshot_storage(&self) -> SnapshotStoragePricing {
        todo!()
    }
}

/// box-backend.ts:72.
pub const BOX_BASE_TEMPLATE: &str = r#"base"#;

/// box-backend.ts:76. One wake attempt, 900,000 ms for the daemon, no resume asks.
pub fn box_budgets() -> LifecycleBudgets {
    todo!()
}

pub const BOX_PAUSE_MS: u64 = 600000;
pub const BOX_RESUME_MS: u64 = 300000;
pub const BOX_CREATE_MS: u64 = 600000;
pub const BOX_SNAPSHOT_MS: u64 = 600000;
pub const BOX_POLL_MS: u64 = 2000;
pub const BOX_RESTORE_MS: u64 = 120000;
pub const BOX_KEY_KEPT_MS: u64 = 86400000;
pub const LOST_ANSWER_MS: u64 = 120000;
pub const LOST_ANSWER_EVERY_MS: u64 = 5000;
pub const IN_PROGRESS_WAIT_MS: u64 = 2000;
pub const BACKSTOP_SLACK_MS: u64 = 60000;
pub const BACKSTOP_RETRY_MS: u64 = 30000;
pub const BACKSTOP_RETRY_MAX_MS: u64 = 300000;
pub const BACKSTOP_CAP_MS: u64 = 20000;
pub const LIMITS_KEPT_MS: u64 = 10000;
pub const REFUSAL_LOG_QUIET_MS: u64 = 60000;
pub const FIREWALL_HOLD_MS: u64 = 30000;
pub const FIREWALL_WATCH_MS: u64 = 5000;
pub const ENV_LANDING_MS: u64 = 60000;
pub const FILE_PUT_MAX: u64 = 5242880;
pub const FILE_GET_MAX: u64 = 4194304;
pub const BOX_INLINE_MAX_MS: u64 = 30000;
/// box-backend.ts:125. Seconds.
pub const TRIAL_TTL_S: u64 = 7200;

/// box-backend.ts:132.
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize)]
pub struct StartCount {
    pub used: f64,
    pub limit: f64,
}

/// The three windows, minute, hour and day.
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize)]
pub struct StartWindows {
    pub minute: StartCount,
    pub hour: StartCount,
    pub day: StartCount,
}

impl StartWindows {
    pub fn get(&self, w: StartWindow) -> StartCount {
        todo!()
    }
}

/// box-backend.ts:139. Every optional field is absent where the read named none. Serialized in `limitsOf`'s key
/// order (tests/fixtures/oracle/box.json `limitsOf[].keys`), which is what `check_key` answers.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BoxLimits {
    pub trial: bool,
    pub plan_key: Option<String>,
    pub plan_usd: Option<f64>,
    pub machines_at_once: Option<f64>,
    pub active_boxes: Option<f64>,
    pub starts: Option<StartWindows>,
    pub sizes: Vec<BoxClassType>,
    pub can_start: bool,
    pub start_blocked_reason: Option<String>,
    pub credit_balance_hours: Option<f64>,
    pub billing_status: Option<String>,
}

/// box-backend.ts:175.
pub fn plan_sizes(trial: bool, plan_usd: Option<f64>) -> Vec<BoxClassType> {
    todo!()
}

/// box-backend.ts:182. `None` where the read named no machine cap.
pub fn place_limits_of(limits: &BoxLimits) -> Option<CloudLimits> {
    todo!()
}

/// box-backend.ts:193. `raw` is `GET /limits`'s body as JSON; a field of the wrong type reads as absent.
pub fn limits_of(raw: &Value) -> BoxLimits {
    todo!()
}

/// box-backend.ts:256. A provider state as a machine state; a word the table does not know reads running.
pub fn state_of(state: &str) -> MachineState {
    todo!()
}

/// box-backend.ts:256 `STATE_MAP[state]`: `None` for a word the table does not know.
pub fn state_map(state: &str) -> Option<MachineState> {
    todo!()
}

/// box-backend.ts:250. The box user's home, which the agent starts every command in.
pub const BOX_WORK_DIR: &str = "/home/user";

/// box-backend.ts:277.
pub const BOX_NAME_MAX: usize = 120;

/// box-backend.ts:301. Lengths count UTF-16 units, as the TS's `.length`.
pub fn box_name(labels: &IndexMap<String, String>) -> String {
    todo!()
}

/// box-backend.ts:319. `None` for an absent name or one wsp did not make.
pub fn box_labels(name: Option<&str>) -> Option<IndexMap<String, String>> {
    todo!()
}

/// box-backend.ts:336.
pub const SNAPSHOT_NAME_MAX: usize = 63;

/// box-backend.ts:337. Fails, as the TS throws, with `EngineError::Other` and the TS's sentence.
pub fn box_snapshot_name(name: &str) -> crate::errors::Result<String> {
    todo!()
}

/// box-backend.ts:345. A UUID, any case: the provider's history ids.
pub fn is_uuid(id: &str) -> bool {
    todo!()
}
