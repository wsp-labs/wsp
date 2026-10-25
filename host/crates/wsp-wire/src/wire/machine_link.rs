// SPDX-License-Identifier: AGPL-3.0-only
//! Port of packages/protocol/src/wire/machine-link.ts.

#![allow(unused_imports)]
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::helpers::ReqId;
use crate::types::{Check, JsNumber, SchemaError};
use crate::views::golden_image::MachineKind;
use crate::views::place::{MachineBind, MachineShare, WorkspaceCopy};
use crate::views::workspace::{MachineFacts, MachineState};
use crate::wire::capabilities::{Capabilities, WorkspaceSize};
use crate::wire::daemon::{CgroupMemory, ProcEntry};
pub use wsp_frames::{ContainerPlacedBy, ProcCost, SysHistoryReply, SysPoint, TranscriptsListReply, TranscriptsReadReply, UsageLogsReply};
use wsp_frames::{False, True};

/// A refusal in the two halves the command line and the app say: what happened, and what to do.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SaidFix {
    pub said: String,
    pub fix: String,
}

/// One run of the kernel's module loader: whether it exited 0 and its stderr, or why it could not run at all.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ModprobeRun {
    Ran { ok: bool, stderr: String },
    Failed { error: String },
}

/// What workspaces_blocked_by reads a computer's kernel through: its platform word, a file read that answers the text
/// or nothing where the file is not there, the module loader, and the effective user (absent reads as not root).
pub struct KernelAt<'a> {
    pub platform: &'a str,
    pub read: &'a dyn Fn(&str) -> Option<String>,
    pub modprobe: &'a dyn Fn(&[&str]) -> ModprobeRun,
    pub euid: Option<u32>,
}

/// The two rooms a vault archive's member rule is read in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VaultRoad {
    Seal,
    Import,
}

pub const MACHINE_PUT_PART_BYTES: i64 = 4194304;
pub const NOT_ON_THIS_ROAD: &str = "not on this road";
pub const NO_IMAGES_HERE: &str = "this computer keeps no images: a workspace here is a copy of the computer itself";
pub const CGROUP_CONTROLLERS_PATH: &str = "/sys/fs/cgroup/cgroup.controllers";
pub const PROC_FILESYSTEMS_PATH: &str = "/proc/filesystems";
pub const MODPROBE_PATH: &str = "/sbin/modprobe";
pub const PLACE_BLOCKED_WORD: &str = "can't run threads";
pub const PLACE_NEEDS_ROOT_LINE: &str =
    "joining a Linux computer needs root, since wsp installs its daemon as a system service; log in as root or use sudo";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum DaemonSupervisor {
    #[serde(rename = "systemd")]
    Systemd,
    #[serde(rename = "entrypoint")]
    Entrypoint,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum MachineSpecOnIdle {
    #[serde(rename = "pause")]
    Pause,
    #[serde(rename = "kill")]
    Kill,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MachineSpec {
    pub kind: MachineKind,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub template: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub from_snapshot: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub cpu: Option<JsNumber>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub mem_mb: Option<JsNumber>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub disk_gb: Option<JsNumber>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present", serialize_with = "crate::types::check::js_record_opt")]
    pub envs: Option<IndexMap<String, String>>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present", serialize_with = "crate::types::check::js_record_opt")]
    pub labels: Option<IndexMap<String, String>>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub on_idle: Option<MachineSpecOnIdle>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub idle_timeout_ms: Option<JsNumber>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub idempotency_key: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub engine: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub copy: Option<WorkspaceCopy>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub shares: Option<Vec<MachineShare>>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub binds: Option<Vec<MachineBind>>,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExecResult {
    pub exit_code: i64,
    pub stdout: String,
    pub stderr: String,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MachineShape {
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub cpu: Option<JsNumber>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub mem_mb: Option<JsNumber>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub disk_gb: Option<JsNumber>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub created_at: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub used_bytes: Option<i64>,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MachineLife {
    pub first_life: bool,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PreviewReach {
    pub url: String,
    pub token: String,
    pub expires_at: JsNumber,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SnapshotRow {
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub name: Option<String>,
    pub size_bytes: JsNumber,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub restored_bytes: Option<JsNumber>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub created_at: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", with = "crate::types::check::double_option")]
    pub parent: Option<Option<String>>,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TemplateRowStatus {
    #[serde(rename = "building")]
    Building,
    #[serde(rename = "ready")]
    Ready,
    #[serde(rename = "failed")]
    Failed,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TemplateRow {
    pub id: String,
    pub name: String,
    pub status: TemplateRowStatus,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub error: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub created_at: Option<String>,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SnapshotStoragePricing {
    pub free_gb: JsNumber,
    pub usd_per_gb_month: JsNumber,
    pub billed_from: String,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LifecycleBudgetsResumeAsks {
    pub every_ms: JsNumber,
    pub for_ms: JsNumber,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LifecycleBudgets {
    pub wake_attempts: i64,
    pub daemon_answers_ms: JsNumber,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub resume_asks: Option<LifecycleBudgetsResumeAsks>,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MachineListRow {
    pub id: String,
    pub state: MachineState,
    #[serde(serialize_with = "crate::types::check::js_record")]
    pub labels: IndexMap<String, String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub size: Option<WorkspaceSize>,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum MachineErrorKind {
    #[serde(rename = "concurrency")]
    Concurrency,
    #[serde(rename = "plan")]
    Plan,
    #[serde(rename = "missing")]
    Missing,
    #[serde(rename = "conflict")]
    Conflict,
    #[serde(rename = "snapshotUnavailable")]
    SnapshotUnavailable,
    #[serde(rename = "transient")]
    Transient,
    #[serde(rename = "auth")]
    Auth,
    #[serde(rename = "unknown")]
    Unknown,
    #[serde(rename = "absent")]
    Absent,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BackendFactsPricing {
    pub default_size: WorkspaceSize,
    pub snapshot_storage: SnapshotStoragePricing,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub builder_disk_gb: Option<JsNumber>,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BackendFactsLifecycle {
    pub budgets: LifecycleBudgets,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BackendFactsBaseTemplates {
    pub sandbox: String,
    pub desktop: String,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BackendFacts {
    pub offer: String,
    pub capabilities: Capabilities,
    pub pricing: BackendFactsPricing,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub lifecycle: Option<BackendFactsLifecycle>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub base_templates: Option<BackendFactsBaseTemplates>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub logins: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub projects: Option<String>,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MachineHandleSeen {
    pub state: MachineState,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub created_at: Option<String>,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MachineHandleRoads {
    pub preview_url: bool,
    pub daemon_answers: bool,
    pub put_bytes: bool,
    pub describe: bool,
    pub facts: bool,
    pub metrics: bool,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MachineHandle {
    pub id: String,
    pub kind: MachineKind,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub stream_url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present", serialize_with = "crate::types::check::js_record_opt")]
    pub labels: Option<IndexMap<String, String>>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub seen: Option<MachineHandleSeen>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub replayed: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub daemon_supervisor: Option<DaemonSupervisor>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub notice: Option<String>,
    pub roads: MachineHandleRoads,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "op")]
pub enum MachineLinkRequest {
    #[serde(rename = "machine.backend", rename_all = "camelCase")]
    MachineBackend { id: ReqId },
    #[serde(rename = "machine.capacity", rename_all = "camelCase")]
    MachineCapacity { id: ReqId },
    #[serde(rename = "machine.checkKey", rename_all = "camelCase")]
    MachineCheckKey { id: ReqId },
    #[serde(rename = "machine.create", rename_all = "camelCase")]
    MachineCreate { id: ReqId, spec: MachineSpec },
    #[serde(rename = "machine.get", rename_all = "camelCase")]
    MachineGet { id: ReqId, machine_id: String },
    #[serde(rename = "machine.list", rename_all = "camelCase")]
    MachineList {
        id: ReqId,
        #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
        labels: Option<IndexMap<String, String>>,
    },
    #[serde(rename = "machine.exec", rename_all = "camelCase")]
    MachineExec {
        id: ReqId,
        machine_id: String,
        cmd: String,
        #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
        timeout_ms: Option<i64>,
        #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
        stdin: Option<String>,
    },
    #[serde(rename = "machine.pause", rename_all = "camelCase")]
    MachinePause { id: ReqId, machine_id: String },
    #[serde(rename = "machine.resume", rename_all = "camelCase")]
    MachineResume { id: ReqId, machine_id: String },
    #[serde(rename = "machine.kill", rename_all = "camelCase")]
    MachineKill { id: ReqId, machine_id: String },
    #[serde(rename = "machine.state", rename_all = "camelCase")]
    MachineState { id: ReqId, machine_id: String },
    #[serde(rename = "machine.describe", rename_all = "camelCase")]
    MachineDescribe { id: ReqId, machine_id: String },
    #[serde(rename = "machine.facts", rename_all = "camelCase")]
    MachineFacts { id: ReqId, machine_id: String },
    #[serde(rename = "machine.metrics", rename_all = "camelCase")]
    MachineMetrics { id: ReqId, machine_id: String },
    #[serde(rename = "machine.daemonAnswers", rename_all = "camelCase")]
    MachineDaemonAnswers {
        id: ReqId,
        machine_id: String,
        #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
        timeout_ms: Option<i64>,
    },
    #[serde(rename = "machine.previewUrl", rename_all = "camelCase")]
    MachinePreviewUrl { id: ReqId, machine_id: String, port: i64 },
    #[serde(rename = "machine.downloadUrl", rename_all = "camelCase")]
    MachineDownloadUrl { id: ReqId, machine_id: String, path: String },
    #[serde(rename = "machine.uploadUrl", rename_all = "camelCase")]
    MachineUploadUrl { id: ReqId, machine_id: String, path: String },
    #[serde(rename = "machine.putBytes", rename_all = "camelCase")]
    MachinePutBytes {
        id: ReqId,
        machine_id: String,
        path: String,
        upload_id: String,
        seq: i64,
        last: bool,
        data: String,
        #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
        timeout_ms: Option<i64>,
    },
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MachineHandleReply {
    pub machine: MachineHandle,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MachineListReply {
    pub machines: Vec<MachineListRow>,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MachineExecReply {
    pub result: ExecResult,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MachineStateReply {
    pub state: MachineState,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MachineShapeReply {
    pub shape: MachineShape,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MachineReading {
    pub state: MachineState,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub cpu: Option<JsNumber>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub mem_mb: Option<JsNumber>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub mem_bytes: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub cpu_usage_usec: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub uptime_ms: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub procs: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub quiet_for_ms: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub address: Option<String>,
    pub cgroup: String,
    pub upper: String,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MachineReadingReply {
    pub reading: MachineReading,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MachineFactsReply {
    pub facts: MachineFacts,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MachineAnswersReply {
    pub answers: bool,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MachineReachReply {
    pub reach: PreviewReach,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MachineUrlReply {
    pub url: String,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum DaemonErrorCode {
    #[serde(rename = "unsupported")]
    Unsupported,
    #[serde(rename = "outside-root")]
    OutsideRoot,
    #[serde(rename = "not-found")]
    NotFound,
    #[serde(rename = "not-a-directory")]
    NotADirectory,
    #[serde(rename = "not-a-file")]
    NotAFile,
    #[serde(rename = "not-a-git-repo")]
    NotAGitRepo,
    #[serde(rename = "bad-request")]
    BadRequest,
    #[serde(rename = "forbidden")]
    Forbidden,
    #[serde(rename = "no-host-cli")]
    NoHostCli,
    #[serde(rename = "no-git-credential")]
    NoGitCredential,
    #[serde(rename = "rate-limited")]
    RateLimited,
    #[serde(rename = "on-default-branch")]
    OnDefaultBranch,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DaemonOkResponse {
    pub id: Option<ReqId>,
    pub ok: True,
    /// zod passthrough: unknown keys kept, in the order read.
    #[serde(flatten, serialize_with = "crate::types::check::js_record")]
    pub rest: IndexMap<String, Value>,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DaemonErrorResponse {
    pub id: Option<ReqId>,
    pub ok: False,
    pub error: String,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub code: Option<DaemonErrorCode>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub kind: Option<MachineErrorKind>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub status: Option<i64>,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum DaemonResponse {
    V0(DaemonOkResponse),
    V1(DaemonErrorResponse),
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SysSampleMem {
    pub used: JsNumber,
    pub total: JsNumber,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SysSampleDisk {
    pub used: JsNumber,
    pub total: JsNumber,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SysSample {
    pub cpu: JsNumber,
    pub load1: JsNumber,
    pub mem: SysSampleMem,
    pub disk: SysSampleDisk,
    pub at: JsNumber,
}

/// A member of a union tagged on `type`: the union writes and reads the tag, so the struct holds none.
impl SysSample {
    pub const TYPE: &str = "sys.sample";
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReadingsAnswer {
    pub points: Vec<SysPoint>,
    pub step_ms: JsNumber,
    pub from: JsNumber,
    pub to: JsNumber,
}
/// A SysSample as it stands alone inside workspace.sys and place.sys: `{"type":"sys.sample", ...}`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum SysSampleFrame {
    #[serde(rename = "sys.sample")]
    SysSample(SysSample),
}

impl Check for SysSampleFrame {
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceSysEvent {
    pub r#type: String,
    pub workspace_id: String,
    pub sample: SysSampleFrame,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaceSysEvent {
    pub r#type: String,
    pub place_id: String,
    pub sample: SysSampleFrame,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProcContainer {
    pub id: String,
    pub name: String,
    pub image: String,
    pub state: String,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub started_at: Option<JsNumber>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub thread: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::wire::helpers::or_none")]
    pub placed_by: Option<ContainerPlacedBy>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub folder: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub project: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub memory: Option<JsNumber>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub cpu: Option<JsNumber>,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProcSnapshot {
    pub at: JsNumber,
    pub daemon: i64,
    pub total: i64,
    pub procs: Vec<ProcEntry>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub seq: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub cost: Option<ProcCost>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub containers: Option<Vec<ProcContainer>>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub cgroups: Option<Vec<CgroupMemory>>,
}

/// A member of a union tagged on `type`: the union writes and reads the tag, so the struct holds none.
impl ProcSnapshot {
    pub const TYPE: &str = "proc.snapshot";
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProcChanges {
    pub at: JsNumber,
    pub daemon: i64,
    pub total: i64,
    pub procs: Vec<ProcEntry>,
    pub gone: Vec<i64>,
    pub seq: i64,
    pub base: i64,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub cost: Option<ProcCost>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub containers: Option<Vec<ProcContainer>>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub cgroups: Option<Vec<CgroupMemory>>,
}

/// A member of a union tagged on `type`: the union writes and reads the tag, so the struct holds none.
impl ProcChanges {
    pub const TYPE: &str = "proc.changes";
}

/// No refinement of its own; checks every nested field that has one.
impl Check for MachineSpec {
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}

/// No refinement of its own; checks every nested field that has one.
impl Check for ExecResult {
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}

/// Refinements zod puts on this shape, each an issue when it fails:
/// - `usedBytes`: int min 0
impl Check for MachineShape {
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}

/// No refinement of its own; checks every nested field that has one.
impl Check for MachineLife {
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}

/// No refinement of its own; checks every nested field that has one.
impl Check for PreviewReach {
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}

/// Refinements zod puts on this shape, each an issue when it fails:
/// - `parent`: absent, null or a value: Option<Option<_>> keeps the three apart
impl Check for SnapshotRow {
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}

/// No refinement of its own; checks every nested field that has one.
impl Check for TemplateRow {
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}

/// No refinement of its own; checks every nested field that has one.
impl Check for SnapshotStoragePricing {
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}

/// Refinements zod puts on this shape, each an issue when it fails:
/// - `wakeAttempts`: int min 1
/// - `daemonAnswersMs`: number min 0 exclusive
impl Check for LifecycleBudgets {
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}

/// No refinement of its own; checks every nested field that has one.
impl Check for LifecycleBudgetsResumeAsks {
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}

/// No refinement of its own; checks every nested field that has one.
impl Check for MachineListRow {
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}

/// Refinements zod puts on this shape, each an issue when it fails:
/// - `logins`: refine: (val, ctx) => { const result = check(val); const setError = () => ctx.addIssue({ code: ZodIssueCode.custom, ...getIssueProperties(val) }); if (typeof Promise !=
/// - `logins`: string min 1
/// - `projects`: refine: (val, ctx) => { const result = check(val); const setError = () => ctx.addIssue({ code: ZodIssueCode.custom, ...getIssueProperties(val) }); if (typeof Promise !=
/// - `projects`: string min 1
impl Check for BackendFacts {
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}

/// No refinement of its own; checks every nested field that has one.
impl Check for BackendFactsPricing {
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}

/// No refinement of its own; checks every nested field that has one.
impl Check for BackendFactsLifecycle {
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}

/// No refinement of its own; checks every nested field that has one.
impl Check for BackendFactsBaseTemplates {
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}

/// No refinement of its own; checks every nested field that has one.
impl Check for MachineHandle {
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}

/// No refinement of its own; checks every nested field that has one.
impl Check for MachineHandleSeen {
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}

/// No refinement of its own; checks every nested field that has one.
impl Check for MachineHandleRoads {
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}

/// No refinement of its own; checks every nested field that has one.
impl Check for MachineLinkRequest {
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}

/// No refinement of its own; checks every nested field that has one.
impl Check for MachineHandleReply {
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}

/// No refinement of its own; checks every nested field that has one.
impl Check for MachineListReply {
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}

/// No refinement of its own; checks every nested field that has one.
impl Check for MachineExecReply {
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}

/// No refinement of its own; checks every nested field that has one.
impl Check for MachineStateReply {
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}

/// No refinement of its own; checks every nested field that has one.
impl Check for MachineShapeReply {
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}

/// Refinements zod puts on this shape, each an issue when it fails:
/// - `memBytes`: int min 0
/// - `cpuUsageUsec`: int min 0
/// - `uptimeMs`: int min 0
/// - `procs`: int min 0
/// - `quietForMs`: int min 0
impl Check for MachineReading {
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}

/// No refinement of its own; checks every nested field that has one.
impl Check for MachineReadingReply {
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}

/// No refinement of its own; checks every nested field that has one.
impl Check for MachineFactsReply {
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}

/// No refinement of its own; checks every nested field that has one.
impl Check for MachineAnswersReply {
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}

/// No refinement of its own; checks every nested field that has one.
impl Check for MachineReachReply {
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}

/// No refinement of its own; checks every nested field that has one.
impl Check for MachineUrlReply {
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}

/// No refinement of its own; checks every nested field that has one.
impl Check for DaemonOkResponse {
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}

/// No refinement of its own; checks every nested field that has one.
impl Check for DaemonErrorResponse {
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}

/// No refinement of its own; checks every nested field that has one.
impl Check for DaemonResponse {
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}

/// Refinements zod puts on this shape, each an issue when it fails:
/// - `type`: literal "sys.sample"
impl Check for SysSample {
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}

/// No refinement of its own; checks every nested field that has one.
impl Check for SysSampleMem {
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}

/// No refinement of its own; checks every nested field that has one.
impl Check for SysSampleDisk {
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}

/// No refinement of its own; checks every nested field that has one.
impl Check for ReadingsAnswer {
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}

/// Refinements zod puts on this shape, each an issue when it fails:
/// - `type`: literal "workspace.sys"
impl Check for WorkspaceSysEvent {
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}

/// Refinements zod puts on this shape, each an issue when it fails:
/// - `type`: literal "place.sys"
impl Check for PlaceSysEvent {
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}

/// Refinements zod puts on this shape, each an issue when it fails:
/// - `placedBy`: catch null: a value that does not parse reads as absent
impl Check for ProcContainer {
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}

/// Refinements zod puts on this shape, each an issue when it fails:
/// - `type`: literal "proc.snapshot"
impl Check for ProcSnapshot {
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}

/// Refinements zod puts on this shape, each an issue when it fails:
/// - `type`: literal "proc.changes"
impl Check for ProcChanges {
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}

// Refinements on the variants of the unions above, each an issue when it fails:
// - MachineLinkRequest::MachineExec.cmd: string max 16384
// - MachineLinkRequest::MachineExec.timeoutMs: int min 0 exclusive
// - MachineLinkRequest::MachineDaemonAnswers.timeoutMs: int min 0 exclusive
// - MachineLinkRequest::MachinePreviewUrl.port: int min 1
// - MachineLinkRequest::MachinePreviewUrl.port: int max 65535
// - MachineLinkRequest::MachinePutBytes.uploadId: string min 1
// - MachineLinkRequest::MachinePutBytes.uploadId: string max 32
// - MachineLinkRequest::MachinePutBytes.uploadId: string regex /^[a-z0-9]+$/
// - MachineLinkRequest::MachinePutBytes.seq: int min 0
// - MachineLinkRequest::MachinePutBytes.timeoutMs: int min 0 exclusive

// machine-link.ts:292
// /** What a fork is refused with when the place it lands on answers that it holds no copy of the snapshot named. A
//  * fork builds the copy it needs only of the image's current version, so this is an older version or a project's
//  * image named at a place it was never built at. */
// export const placeHoldsNoImageLine = (place: string, image: string): string =>
//   `${place} holds no copy of ${image}; a fork there builds a copy first only of your image's current version`;
pub fn place_holds_no_image_line(place: &str, image: &str) -> String {
    todo!()
}
// machine-link.ts:298
// /** What a remove of a computer is refused with while a fork or a project there holds work no remote has, each named
//  * with what it holds, a project folder by its checkout's path there: the roads that keep it, and the flag that takes
//  * it anyway. */
// export const placeUnsavedRefusal = (place: string, unsaved: readonly string[]): { said: string; fix: string } => ({
//   said: `${place} holds work no remote has, which a remove would lose: ${unsaved.join("; ")}`,
//   fix: `Keep a fork's work with wsp export or by pushing its branch, and copy a project folder's off ${place} from the path named; then remove ${place} again, or wsp remove ${place} --force removes it anyway.`,
// });
pub fn place_unsaved_refusal(place: &str, unsaved: &[String]) -> SaidFix {
    todo!()
}
// machine-link.ts:306
// /** What a remove of a computer is refused with while a fork or a project stands on it and its link is down: the forks
//  * are deleted and the project folders read over that link, so nothing about them can be done or known until it is
//  * back. A computer that will never answer again is forgotten instead. */
// export const placeAwayRefusal = (place: string, sentence: string): { said: string; fix: string } => ({
//   said: `${sentence}, and its forks and projects go over its link`,
//   fix: `Turn ${place} on and remove it again once it answers; if it never will, wsp remove ${place} --forget takes it, its forks, projects and threads out of this wsp, with nothing done on it beyond a try at wsp's own leave over ssh.`,
// });
pub fn place_away_refusal(place: &str, sentence: &str) -> SaidFix {
    todo!()
}
// machine-link.ts:314
// /** What a forget of a computer is refused with while its link answers: a remove takes wsp off it. */
// export const placeForgetAnswersRefusal = (place: string): { said: string; fix: string } => ({
//   said: `${place} is answering, so there is nothing to forget`,
//   fix: `wsp remove ${place} takes it out and takes wsp off it.`,
// });
pub fn place_forget_answers_refusal(place: &str) -> SaidFix {
    todo!()
}
// machine-link.ts:320
// /** What a remove that stopped part way through the forks and projects on a computer says: why, and which of them had
//  * already gone, since those are not coming back. */
// export const placeDropStoppedLine = (why: string, went: readonly string[]): string => (went.length === 0 ? why : `${why}; ${went.join(", ")} had already gone with the remove`);
pub fn place_drop_stopped_line(why: &str, went: &[String]) -> String {
    todo!()
}
// machine-link.ts:324
// /** What a remove of a project is refused with while its checkout on a computer of the person's holds work no remote
//  * has, named by its path there. */
// export const projectUnsavedRefusal = (project: string, unsaved: string): { said: string; fix: string } => ({
//   said: `${unsaved}, which removing ${project} would lose`,
//   fix: `Copy that work off its computer from the path named or push its branches, then remove ${project} again; wsp projects remove ${project} --force removes it anyway.`,
// });
pub fn project_unsaved_refusal(project: &str, unsaved: &str) -> SaidFix {
    todo!()
}
// machine-link.ts:331
// /** How an unsaved read names a project folder: by its project and the checkout's path on the computer holding it,
//  * which is where a person goes to keep what is there. */
// export const projectFolderNamed = (project: string, checkout: string): string => `${project} at ${checkout}`;
pub fn project_folder_named(project: &str, checkout: &str) -> String {
    todo!()
}
// machine-link.ts:363
// /** What a fork aimed at a place this host no longer holds a record for is refused with. Every computer on the
//  * list forks, so the only way to reach this is a record that went between the word being read and the fork being
//  * asked for: a remove, or a store another process wrote. */
// export const placeForksNowhereLine = (place: string): string => `${place} is no longer a place in this wsp, so nothing forks there`;
pub fn place_forks_nowhere_line(place: &str) -> String {
    todo!()
}
// machine-link.ts:368
// /** What a verb aimed at the bare computer a place is, rather than at a workspace forked on it, is refused with. A
//  * place holds its facts and its forks; the forks are the workspaces, so the refusal names the computer and the one
//  * road to a workspace there. */
// export const placeNotAWorkspaceLine = (place: string): string => `${place} is a computer you joined, not a workspace; its forks are the workspaces`;
pub fn place_not_a_workspace_line(place: &str) -> String {
    todo!()
}
// machine-link.ts:372
// export const placeNotAWorkspaceFix = (place: string): string => `Name a machine on it: wsp threads lists what runs on ${place}.`;
pub fn place_not_a_workspace_fix(place: &str) -> String {
    todo!()
}
// machine-link.ts:374
// /** What a caller asking for the road to a workspace's own daemon is told, where that workspace runs none: a
//  * workspace on a computer somebody owns is that computer's directories under the computer's own daemon, and that
//  * daemon answers its files and its git through the host. Said rather than a route minted to a port nothing listens
//  * on, which is what the panes and the relay read before. */
// export const placeServesDaemonLine = (workspace: string, computer: string): string =>
//   `${workspace} has no daemon of its own: ${computer} answers its files and git through this host`;
pub fn place_serves_daemon_line(workspace: &str, computer: &str) -> String {
    todo!()
}
// machine-link.ts:381
// /** What a watch of the ports or of the load is refused with on a workspace whose computer answers its daemon
//  * frames: both readings are that whole computer's, and one workspace reading them would read another workspace's
//  * listeners and load as its own. A pane asks for both on every link it opens and takes a refusal of either. */
// export const placeWatchesItselfLine = (computer: string): string =>
//   `${computer} watches its own ports and load, which are that computer's rather than one workspace's`;
pub fn place_watches_itself_line(computer: &str) -> String {
    todo!()
}
// machine-link.ts:387
// /** What a watch, read or signal of processes is refused with on a workspace whose computer answers its daemon
//  * frames: that daemon acts on any pid on the computer, the computer's own and every other workspace's, so none of
//  * them goes up its link from one workspace's channel. The Processes pane shows it in place of the table. */
// export const forkProcsUnreadLine = (workspace: string, computer: string): string =>
//   `${workspace}'s processes on ${computer} are not readable from here yet`;
pub fn fork_procs_unread_line(workspace: &str, computer: &str) -> String {
    todo!()
}
// machine-link.ts:393
// /** What every other op is refused with on that workspace's channel: the frames that computer's daemon answers
//  * inside the workspace they name are its shells, its files and its git, and every other op there runs on the
//  * computer itself. */
// export const forkOpRefusedLine = (op: string, workspace: string, computer: string): string =>
//   `${computer} answers ${workspace}'s shells, files and git from here, not ${op}`;
pub fn fork_op_refused_line(op: &str, workspace: &str, computer: &str) -> String {
    todo!()
}
// machine-link.ts:402
// export const localRunsOneFix = (workspace: string): string => `Use ${workspace}, or name a computer that forks: wsp computers.`;
pub fn local_runs_one_fix(workspace: &str) -> String {
    todo!()
}
// machine-link.ts:404
// /** What a computer's kernel must have before wsp runs workspaces on it, asked in this order so the reason a person
//  * reads is the first thing missing rather than the last. `read` answers a file's text or nothing when it is not
//  * there; `modprobe` runs the kernel's module loader; `euid` is the effective user the check runs as. Nothing here
//  * touches a disk: the join typed at a box reads its own files through this rule, and its steps and words are the
//  * daemon doctor's (`daemon/crates/wsp-runtime/src/doctor.rs`), so what the join says and what a create does
//  * cannot part ways. */
// export function workspacesBlockedBy(at: { platform: string; read: (path: string) => string | undefined; modprobe: ModprobeRun; euid?: number }): string | undefined {
//   if (at.platform !== "linux") return "wsp runs workspaces on a Linux computer";
//   const controllers = at.read(CGROUP_CONTROLLERS_PATH);
//   if (controllers === undefined) {
//     return "this computer mounts cgroup v1 at /sys/fs/cgroup, and wsp runs workspaces on cgroup v2 alone: boot it with systemd.unified_cgroup_hierarchy=1";
//   }
//   const has = new Set(controllers.split(/\s+/));
//   for (const wanted of ["memory", "cpu"]) if (!has.has(wanted)) return `this computer's cgroup root offers no ${wanted} controller, which wsp needs to run workspaces here`;
//   const overlay = overlayMissing(at.read, at.modprobe);
//   if (overlay !== undefined) return overlay;
//   if (at.euid !== 0) return "wsp runs workspaces on this computer as root, and this daemon is not root";
//   return undefined;
// }
pub fn workspaces_blocked_by(at: &KernelAt<'_>) -> Option<String> {
    todo!()
}
// machine-link.ts:453
// /** What a join of a computer whose kernel cannot boot the image is refused with, in the one sentence the daemon's
//  * own doctor named the reason in. A computer that cannot boot your image is not a place, so the join stops here
//  * and nothing is written on it. */
// export const placeCannotBootLine = (place: string, reason?: string): string =>
//   reason === undefined ? `${place} cannot run wsp workspaces, so it cannot be a place` : `${place} cannot run wsp workspaces: ${reason.replace(/this computer('s)?/g, (_, owns) => (owns === undefined ? "it" : "its"))}`;
pub fn place_cannot_boot_line(place: &str, reason: Option<&str>) -> String {
    todo!()
}
// machine-link.ts:459
// /** The same sentence off a computer's own report, or nothing while it runs workspaces: one reading for the join's
//  * refusal, the row, and every act that runs inside a copy there. */
// export const placeBlocked = (place: string, report: Pick<PlaceReport, "runsWorkspaces" | "workspacesBlocked">): string | undefined =>
//   report.runsWorkspaces ? undefined : placeCannotBootLine(place, report.workspacesBlocked);
pub fn place_blocked(place: &str, runs_workspaces: bool, workspaces_blocked: Option<&str>) -> Option<String> {
    todo!()
}
// machine-link.ts:464
// /** What a computer's row says while the threads on that computer itself get no wsp tools: its daemon could not open
//  * the door they reach the tools through, in its own words, and the restart that opens it again, of the unit the
//  * daemon says it runs under. An update does not do it: one of a daemon already current sends no binary and restarts
//  * nothing. A daemon in no unit the join writes names none, and is added again to get one. Nothing from a daemon older
//  * than that door, which the row's behind word already answers, or from one whose door stands. */
// export const placeNoToolsLine = (place: string, report: Pick<PlaceReport, "wspDoor" | "wspDoorBlocked" | "daemonUnit">): string | undefined =>
//   report.wspDoor !== false
//     ? undefined
//     : `threads on ${place} get no wsp tools: ${report.wspDoorBlocked ?? "its daemon opened no door for them"}; ${report.daemonUnit === undefined ? `remove ${place} and add it again` : `restart wsp's daemon there with systemctl restart ${report.daemonUnit}`}`;
pub fn place_no_tools_line(
    place: &str,
    wsp_door: Option<bool>,
    wsp_door_blocked: Option<&str>,
    daemon_unit: Option<&str>,
) -> Option<String> {
    todo!()
}
// machine-link.ts:482
// /** What a word that names no place this host holds is refused with, naming the ones it does. */
// export const noSuchPlaceRefusal = (word: string, held: readonly string[]): string => `no place named ${word}; you have ${held.join(", ")}`;
pub fn no_such_place_refusal(word: &str, held: &[String]) -> String {
    todo!()
}
// machine-link.ts:485
// /** Whether a row is a computer somebody joined to this wsp: a computer, and not the one the host runs on, whose
//  * files and threads are that host's own. The one reading, so the road that runs on the link, the road at the
//  * terminal and the list of places a fork could stand on cannot disagree about which rows are those computers. */
// export const isJoinedComputer = (place: { id: string; kind: string }): boolean => place.kind === "computer" && place.id !== HERE_PLACE_ID;
pub fn is_joined_computer(id: &str, kind: &str) -> bool {
    todo!()
}
// machine-link.ts:490
// /** What a build of the image at a place that cannot take one is refused with: a copy needs a builder forked there
//  * and that builder's disk copied, and a computer somebody joined does neither. */
// export const placeBuildsNoImageLine = (place: string): string =>
//   `${place} takes no copy of your image: a copy is built by forking a machine there and copying its disk, and ${place} does neither`;
pub fn place_builds_no_image_line(place: &str) -> String {
    todo!()
}
// machine-link.ts:499
// /** What a vault archive is refused with: where the reading stopped, why, and what that refusal did, which is not
//  * the same on the two roads. The archive is refused whole, since a builder that wrote one member nobody asked for
//  * wrote every other member too. */
// export const vaultMemberRefusal = (road: VaultRoad, member: string, why: string): string =>
//   `the image's sign-in archive is refused at ${member}: ${why}; ${road === "seal" ? "the seal is refused and no version is recorded" : "nothing of it was imported"}`;
pub fn vault_member_refusal(road: VaultRoad, member: &str, why: &str) -> String {
    todo!()
}
// machine-link.ts:505
// /** What a copy is refused with for a record whose vault kept no path list: sealed before the record held which
//  * paths its sign-ins live at, so no other place can tell a member the seal asked for from one it did not. */
// export const vaultUnlistedRefusal = (name: string, version: number): string =>
//   `${name} v${version} was sealed before its record kept which paths its sign-ins live at, so no other place can check its archive against them; cut the next version`;
pub fn vault_unlisted_refusal(name: &str, version: i64) -> String {
    todo!()
}
// machine-link.ts:693
// /** The snapshot a proc.changes frame leaves, rows by pid as the daemon lists them, the containers it carried or else
//  * those held, and the cgroups' memory it carried, never an older tick's; undefined where the frame does not
//  * follow the snapshot held, which a client answers by watching again for a whole one. */
// export function applyProcChanges(snapshot: ProcSnapshot, changes: ProcChanges): ProcSnapshot | undefined {
//   if (changes.base !== snapshot.seq) return undefined;
//   const rows = new Map(snapshot.procs.map(p => [p.pid, p]));
//   for (const pid of changes.gone) rows.delete(pid);
//   for (const p of changes.procs) rows.set(p.pid, p);
//   return { type: "proc.snapshot", at: changes.at, daemon: changes.daemon, total: changes.total, procs: [...rows.values()].sort((a, b) => a.pid - b.pid), seq: changes.seq, ...(changes.cost === undefined ? {} : { cost: changes.cost }), ...((changes.containers ?? snapshot.containers) === undefined ? {} : { containers: changes.containers ?? snapshot.containers }), ...(changes.cgroups === undefined ? {} : { cgroups: changes.cgroups }) };
// }
pub fn apply_proc_changes(snapshot: &ProcSnapshot, changes: &ProcChanges) -> Option<ProcSnapshot> {
    todo!()
}

/// Reads one machine-link request as zod's discriminated union on `op` reads it, so a refusal's first issue is zod's: a value
/// that is not an object is one invalid_type issue at the root (expected "object"); an absent or unknown `op` is
/// one invalid_union_discriminator issue on ["op"] whose detail `options` lists every op of MachineLinkRequest in order and
/// whose message is "Invalid discriminator value. Expected 'a' | 'b' | ..."; otherwise the variant's first failing
/// field, then its Check. Unknown keys are dropped.
pub fn parse_machine_link_request(frame: &Value) -> Result<MachineLinkRequest, SchemaError> {
    let _ = frame;
    todo!()
}

/// Every op MachineLinkRequest holds, in the TS union's order.
pub const MACHINE_LINK_OPS: &[&str] = &[
    "machine.backend",
    "machine.capacity",
    "machine.checkKey",
    "machine.create",
    "machine.get",
    "machine.list",
    "machine.exec",
    "machine.pause",
    "machine.resume",
    "machine.kill",
    "machine.state",
    "machine.describe",
    "machine.facts",
    "machine.metrics",
    "machine.daemonAnswers",
    "machine.previewUrl",
    "machine.downloadUrl",
    "machine.uploadUrl",
    "machine.putBytes",
];

// The shapes wsp-frames holds read every bound while deserializing, so each Check has nothing left to refuse.
impl Check for ProcCost {
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}
impl Check for SysHistoryReply {
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}
impl Check for SysPoint {
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}
impl Check for TranscriptsListReply {
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}
impl Check for TranscriptsReadReply {
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}
impl Check for UsageLogsReply {
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}

// A word list zod reads with z.enum: serde refuses any other word, so nothing is left to check.
impl Check for DaemonSupervisor {
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}
impl Check for MachineSpecOnIdle {
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}
impl Check for TemplateRowStatus {
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}
impl Check for MachineErrorKind {
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}
impl Check for DaemonErrorCode {
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}

/// machine.backend answers the backend's facts and machine.capacity its room: the same shapes, by the same name
/// the TS gives them (machine-link.ts:245-246).
pub type MachineBackendReply = BackendFacts;
pub type MachineCapacityReply = crate::wire::capabilities::PlaceCapacity;
