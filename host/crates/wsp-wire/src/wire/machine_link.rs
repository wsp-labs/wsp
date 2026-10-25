// SPDX-License-Identifier: AGPL-3.0-only
//! Port of packages/protocol/src/wire/machine-link.ts.

#![allow(unused_imports)]
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::helpers::ReqId;
use super::helpers::{literal_word, op_discriminator};
use crate::types::check::parse;
use crate::types::check::{num_above, num_at_least, num_at_most, refine, regex_check, text_at_least, text_at_most};
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub template: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub from_snapshot: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cpu: Option<JsNumber>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mem_mb: Option<JsNumber>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub disk_gb: Option<JsNumber>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub envs: Option<IndexMap<String, String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub labels: Option<IndexMap<String, String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub on_idle: Option<MachineSpecOnIdle>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub idle_timeout_ms: Option<JsNumber>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub idempotency_key: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub engine: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub copy: Option<WorkspaceCopy>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shares: Option<Vec<MachineShare>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cpu: Option<JsNumber>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mem_mb: Option<JsNumber>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub disk_gb: Option<JsNumber>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub created_at: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    pub size_bytes: JsNumber,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub restored_bytes: Option<JsNumber>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resume_asks: Option<LifecycleBudgetsResumeAsks>,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MachineListRow {
    pub id: String,
    pub state: MachineState,
    pub labels: IndexMap<String, String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lifecycle: Option<BackendFactsLifecycle>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub base_templates: Option<BackendFactsBaseTemplates>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub logins: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub projects: Option<String>,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MachineHandleSeen {
    pub state: MachineState,
    #[serde(default, skip_serializing_if = "Option::is_none")]
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stream_url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub labels: Option<IndexMap<String, String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub seen: Option<MachineHandleSeen>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub replayed: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub daemon_supervisor: Option<DaemonSupervisor>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
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
        #[serde(default, skip_serializing_if = "Option::is_none")]
        labels: Option<IndexMap<String, String>>,
    },
    #[serde(rename = "machine.exec", rename_all = "camelCase")]
    MachineExec {
        id: ReqId,
        machine_id: String,
        cmd: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        timeout_ms: Option<i64>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
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
        #[serde(default, skip_serializing_if = "Option::is_none")]
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
        #[serde(default, skip_serializing_if = "Option::is_none")]
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cpu: Option<JsNumber>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mem_mb: Option<JsNumber>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mem_bytes: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cpu_usage_usec: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub uptime_ms: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub procs: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub quiet_for_ms: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
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
    #[serde(flatten)]
    pub rest: IndexMap<String, Value>,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DaemonErrorResponse {
    pub id: Option<ReqId>,
    pub ok: False,
    pub error: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub code: Option<DaemonErrorCode>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kind: Option<MachineErrorKind>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
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
        match self {
            SysSampleFrame::SysSample(sample) => sample.check(),
        }
    }

    fn describe(path: &[Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) if key.as_str() == Some("type") && rest.is_empty() => {
                Some(crate::types::check::Zod::Literal(|| Value::from("sys.sample")))
            }
            Some((key, rest)) => match key.as_str()? {
                "cpu" | "load1" | "at" => <JsNumber as Check>::describe(rest),
                _ => None,
            },
        }
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub started_at: Option<JsNumber>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub thread: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::wire::helpers::or_none")]
    pub placed_by: Option<ContainerPlacedBy>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub folder: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub memory: Option<JsNumber>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cpu: Option<JsNumber>,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProcSnapshot {
    pub at: JsNumber,
    pub daemon: i64,
    pub total: i64,
    pub procs: Vec<ProcEntry>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub seq: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cost: Option<ProcCost>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub containers: Option<Vec<ProcContainer>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cost: Option<ProcCost>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub containers: Option<Vec<ProcContainer>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cgroups: Option<Vec<CgroupMemory>>,
}

/// A member of a union tagged on `type`: the union writes and reads the tag, so the struct holds none.
impl ProcChanges {
    pub const TYPE: &str = "proc.changes";
}

/// No refinement of its own; checks every nested field that has one.
impl Check for MachineSpec {
    fn check(&self) -> Result<(), SchemaError> {
        self.copy.check().map_err(|e| e.under("copy"))?;
        self.shares.check().map_err(|e| e.under("shares"))?;
        self.binds.check().map_err(|e| e.under("binds"))?;
        Ok(())
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "kind" => <MachineKind as Check>::describe(rest),
                "template" => <Option<String> as Check>::describe(rest),
                "fromSnapshot" => <Option<String> as Check>::describe(rest),
                "cpu" => <Option<JsNumber> as Check>::describe(rest),
                "memMb" => <Option<JsNumber> as Check>::describe(rest),
                "diskGb" => <Option<JsNumber> as Check>::describe(rest),
                "envs" => <Option<IndexMap<String, String>> as Check>::describe(rest),
                "labels" => <Option<IndexMap<String, String>> as Check>::describe(rest),
                "onIdle" => <Option<MachineSpecOnIdle> as Check>::describe(rest),
                "idleTimeoutMs" => <Option<JsNumber> as Check>::describe(rest),
                "idempotencyKey" => <Option<String> as Check>::describe(rest),
                "engine" => <Option<bool> as Check>::describe(rest),
                "copy" => <Option<WorkspaceCopy> as Check>::describe(rest),
                "shares" => <Option<Vec<MachineShare>> as Check>::describe(rest),
                "binds" => <Option<Vec<MachineBind>> as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

/// No refinement of its own; checks every nested field that has one.
impl Check for ExecResult {
    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "exitCode" => <i64 as Check>::describe(rest),
                "stdout" => <String as Check>::describe(rest),
                "stderr" => <String as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

/// Refinements zod puts on this shape, each an issue when it fails:
/// - `usedBytes`: int min 0
impl Check for MachineShape {
    fn check(&self) -> Result<(), SchemaError> {
        if let Some(v) = self.used_bytes {
            num_at_least(v as f64, 0.0).map_err(|e| e.under("usedBytes"))?;
        }
        Ok(())
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "cpu" => <Option<JsNumber> as Check>::describe(rest),
                "memMb" => <Option<JsNumber> as Check>::describe(rest),
                "diskGb" => <Option<JsNumber> as Check>::describe(rest),
                "createdAt" => <Option<String> as Check>::describe(rest),
                "usedBytes" => <Option<i64> as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

/// No refinement of its own; checks every nested field that has one.
impl Check for MachineLife {
    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "firstLife" => <bool as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

/// No refinement of its own; checks every nested field that has one.
impl Check for PreviewReach {
    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "url" => <String as Check>::describe(rest),
                "token" => <String as Check>::describe(rest),
                "expiresAt" => <JsNumber as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

/// Refinements zod puts on this shape, each an issue when it fails:
/// - `parent`: absent, null or a value: Option<Option<_>> keeps the three apart
impl Check for SnapshotRow {
    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "id" => <String as Check>::describe(rest),
                "name" => <Option<String> as Check>::describe(rest),
                "sizeBytes" => <JsNumber as Check>::describe(rest),
                "restoredBytes" => <Option<JsNumber> as Check>::describe(rest),
                "createdAt" => <Option<String> as Check>::describe(rest),
                "parent" => <Option<Option<String>> as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

/// No refinement of its own; checks every nested field that has one.
impl Check for TemplateRow {
    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "id" => <String as Check>::describe(rest),
                "name" => <String as Check>::describe(rest),
                "status" => <TemplateRowStatus as Check>::describe(rest),
                "error" => <Option<String> as Check>::describe(rest),
                "createdAt" => <Option<String> as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

/// No refinement of its own; checks every nested field that has one.
impl Check for SnapshotStoragePricing {
    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "freeGb" => <JsNumber as Check>::describe(rest),
                "usdPerGbMonth" => <JsNumber as Check>::describe(rest),
                "billedFrom" => <String as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

/// Refinements zod puts on this shape, each an issue when it fails:
/// - `wakeAttempts`: int min 1
/// - `daemonAnswersMs`: number min 0 exclusive
impl Check for LifecycleBudgets {
    fn check(&self) -> Result<(), SchemaError> {
        num_at_least(self.wake_attempts as f64, 1.0).map_err(|e| e.under("wakeAttempts"))?;
        num_above(self.daemon_answers_ms.0, 0.0).map_err(|e| e.under("daemonAnswersMs"))?;
        Ok(())
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "wakeAttempts" => <i64 as Check>::describe(rest),
                "daemonAnswersMs" => <JsNumber as Check>::describe(rest),
                "resumeAsks" => <Option<LifecycleBudgetsResumeAsks> as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

/// No refinement of its own; checks every nested field that has one.
impl Check for LifecycleBudgetsResumeAsks {
    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "everyMs" => <JsNumber as Check>::describe(rest),
                "forMs" => <JsNumber as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

/// No refinement of its own; checks every nested field that has one.
impl Check for MachineListRow {
    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "id" => <String as Check>::describe(rest),
                "state" => <MachineState as Check>::describe(rest),
                "labels" => <IndexMap<String, String> as Check>::describe(rest),
                "size" => <Option<WorkspaceSize> as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

/// Refinements zod puts on this shape, each an issue when it fails:
/// - `logins`: refine: (val, ctx) => { const result = check(val); const setError = () => ctx.addIssue({ code: ZodIssueCode.custom, ...getIssueProperties(val) }); if (typeof Promise !=
/// - `logins`: string min 1
/// - `projects`: refine: (val, ctx) => { const result = check(val); const setError = () => ctx.addIssue({ code: ZodIssueCode.custom, ...getIssueProperties(val) }); if (typeof Promise !=
/// - `projects`: string min 1
impl Check for BackendFacts {
    fn check(&self) -> Result<(), SchemaError> {
        self.capabilities.check().map_err(|e| e.under("capabilities"))?;
        self.lifecycle.check().map_err(|e| e.under("lifecycle"))?;
        if let Some(logins) = &self.logins {
            text_at_least(logins, 1).map_err(|e| e.under("logins"))?;
            refine(crate::wire::limits::is_plain_path(logins), "an absolute path on the computer").map_err(|e| e.under("logins"))?;
        }
        if let Some(projects) = &self.projects {
            text_at_least(projects, 1).map_err(|e| e.under("projects"))?;
            refine(crate::wire::limits::is_plain_path(projects), "an absolute path on the computer").map_err(|e| e.under("projects"))?;
        }
        Ok(())
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "offer" => <String as Check>::describe(rest),
                "capabilities" => <Capabilities as Check>::describe(rest),
                "pricing" => <BackendFactsPricing as Check>::describe(rest),
                "lifecycle" => <Option<BackendFactsLifecycle> as Check>::describe(rest),
                "baseTemplates" => <Option<BackendFactsBaseTemplates> as Check>::describe(rest),
                "logins" => <Option<String> as Check>::describe(rest),
                "projects" => <Option<String> as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

/// No refinement of its own; checks every nested field that has one.
impl Check for BackendFactsPricing {
    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "defaultSize" => <WorkspaceSize as Check>::describe(rest),
                "snapshotStorage" => <SnapshotStoragePricing as Check>::describe(rest),
                "builderDiskGb" => <Option<JsNumber> as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

/// No refinement of its own; checks every nested field that has one.
impl Check for BackendFactsLifecycle {
    fn check(&self) -> Result<(), SchemaError> {
        self.budgets.check().map_err(|e| e.under("budgets"))?;
        Ok(())
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "budgets" => <LifecycleBudgets as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

/// No refinement of its own; checks every nested field that has one.
impl Check for BackendFactsBaseTemplates {
    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "sandbox" => <String as Check>::describe(rest),
                "desktop" => <String as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

/// No refinement of its own; checks every nested field that has one.
impl Check for MachineHandle {
    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "id" => <String as Check>::describe(rest),
                "kind" => <MachineKind as Check>::describe(rest),
                "streamUrl" => <Option<String> as Check>::describe(rest),
                "labels" => <Option<IndexMap<String, String>> as Check>::describe(rest),
                "seen" => <Option<MachineHandleSeen> as Check>::describe(rest),
                "replayed" => <Option<bool> as Check>::describe(rest),
                "daemonSupervisor" => <Option<DaemonSupervisor> as Check>::describe(rest),
                "notice" => <Option<String> as Check>::describe(rest),
                "roads" => <MachineHandleRoads as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

/// No refinement of its own; checks every nested field that has one.
impl Check for MachineHandleSeen {
    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "state" => <MachineState as Check>::describe(rest),
                "createdAt" => <Option<String> as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

/// No refinement of its own; checks every nested field that has one.
impl Check for MachineHandleRoads {
    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "previewUrl" => <bool as Check>::describe(rest),
                "daemonAnswers" => <bool as Check>::describe(rest),
                "putBytes" => <bool as Check>::describe(rest),
                "describe" => <bool as Check>::describe(rest),
                "facts" => <bool as Check>::describe(rest),
                "metrics" => <bool as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

/// No refinement of its own; checks every nested field that has one.
impl Check for MachineLinkRequest {
    fn check(&self) -> Result<(), SchemaError> {
        match self {
            MachineLinkRequest::MachineCreate { spec, .. } => spec.check().map_err(|e| e.under("spec"))?,
            MachineLinkRequest::MachineExec { cmd, timeout_ms, .. } => {
                text_at_most(cmd, crate::wire::limits::EXEC_BODY_MAX as usize).map_err(|e| e.under("cmd"))?;
                if let Some(v) = timeout_ms {
                    num_above(*v as f64, 0.0).map_err(|e| e.under("timeoutMs"))?;
                }
            }
            MachineLinkRequest::MachineDaemonAnswers { timeout_ms, .. } => {
                if let Some(v) = timeout_ms {
                    num_above(*v as f64, 0.0).map_err(|e| e.under("timeoutMs"))?;
                }
            }
            MachineLinkRequest::MachinePreviewUrl { port, .. } => {
                num_at_least(*port as f64, 1.0).map_err(|e| e.under("port"))?;
                num_at_most(*port as f64, 65535.0).map_err(|e| e.under("port"))?;
            }
            MachineLinkRequest::MachinePutBytes { upload_id, seq, timeout_ms, .. } => {
                text_at_least(upload_id, 1).map_err(|e| e.under("uploadId"))?;
                text_at_most(upload_id, 32).map_err(|e| e.under("uploadId"))?;
                regex_check(!upload_id.is_empty() && upload_id.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit()))
                    .map_err(|e| e.under("uploadId"))?;
                num_at_least(*seq as f64, 0.0).map_err(|e| e.under("seq"))?;
                if let Some(v) = timeout_ms {
                    num_above(*v as f64, 0.0).map_err(|e| e.under("timeoutMs"))?;
                }
            }
            _ => {}
        }
        Ok(())
    }

    fn describe(path: &[Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "op" if rest.is_empty() => Some(crate::types::check::Zod::Tags("op", MACHINE_LINK_OPS)),
                "id" => crate::types::check::leaf(rest, crate::types::check::Zod::Union),
                "machineId" | "path" | "uploadId" | "cmd" | "stdin" | "data" => <String as Check>::describe(rest),
                "timeoutMs" | "seq" | "port" => <i64 as Check>::describe(rest),
                "last" => <bool as Check>::describe(rest),
                "spec" => <MachineSpec as Check>::describe(rest),
                "labels" => <Option<IndexMap<String, String>> as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

/// No refinement of its own; checks every nested field that has one.
impl Check for MachineHandleReply {
    fn check(&self) -> Result<(), SchemaError> {
        self.machine.check().map_err(|e| e.under("machine"))?;
        Ok(())
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "machine" => <MachineHandle as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

/// No refinement of its own; checks every nested field that has one.
impl Check for MachineListReply {
    fn check(&self) -> Result<(), SchemaError> {
        self.machines.check().map_err(|e| e.under("machines"))?;
        Ok(())
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "machines" => <Vec<MachineListRow> as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

/// No refinement of its own; checks every nested field that has one.
impl Check for MachineExecReply {
    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "result" => <ExecResult as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

/// No refinement of its own; checks every nested field that has one.
impl Check for MachineStateReply {
    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "state" => <MachineState as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

/// No refinement of its own; checks every nested field that has one.
impl Check for MachineShapeReply {
    fn check(&self) -> Result<(), SchemaError> {
        self.shape.check().map_err(|e| e.under("shape"))?;
        Ok(())
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "shape" => <MachineShape as Check>::describe(rest),
                _ => None,
            },
        }
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
        if let Some(v) = self.mem_bytes {
            num_at_least(v as f64, 0.0).map_err(|e| e.under("memBytes"))?;
        }
        if let Some(v) = self.cpu_usage_usec {
            num_at_least(v as f64, 0.0).map_err(|e| e.under("cpuUsageUsec"))?;
        }
        if let Some(v) = self.uptime_ms {
            num_at_least(v as f64, 0.0).map_err(|e| e.under("uptimeMs"))?;
        }
        if let Some(v) = self.procs {
            num_at_least(v as f64, 0.0).map_err(|e| e.under("procs"))?;
        }
        if let Some(v) = self.quiet_for_ms {
            num_at_least(v as f64, 0.0).map_err(|e| e.under("quietForMs"))?;
        }
        Ok(())
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "state" => <MachineState as Check>::describe(rest),
                "cpu" => <Option<JsNumber> as Check>::describe(rest),
                "memMb" => <Option<JsNumber> as Check>::describe(rest),
                "memBytes" => <Option<i64> as Check>::describe(rest),
                "cpuUsageUsec" => <Option<i64> as Check>::describe(rest),
                "uptimeMs" => <Option<i64> as Check>::describe(rest),
                "procs" => <Option<i64> as Check>::describe(rest),
                "quietForMs" => <Option<i64> as Check>::describe(rest),
                "address" => <Option<String> as Check>::describe(rest),
                "cgroup" => <String as Check>::describe(rest),
                "upper" => <String as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

/// No refinement of its own; checks every nested field that has one.
impl Check for MachineReadingReply {
    fn check(&self) -> Result<(), SchemaError> {
        self.reading.check().map_err(|e| e.under("reading"))?;
        Ok(())
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "reading" => <MachineReading as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

/// No refinement of its own; checks every nested field that has one.
impl Check for MachineFactsReply {
    fn check(&self) -> Result<(), SchemaError> {
        self.facts.check().map_err(|e| e.under("facts"))?;
        Ok(())
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "facts" => <MachineFacts as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

/// No refinement of its own; checks every nested field that has one.
impl Check for MachineAnswersReply {
    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "answers" => <bool as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

/// No refinement of its own; checks every nested field that has one.
impl Check for MachineReachReply {
    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "reach" => <PreviewReach as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

/// No refinement of its own; checks every nested field that has one.
impl Check for MachineUrlReply {
    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "url" => <String as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

/// No refinement of its own; checks every nested field that has one.
impl Check for DaemonOkResponse {
    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "id" => crate::types::check::leaf(rest, crate::types::check::Zod::Union),
                "ok" => crate::types::check::leaf(rest, crate::types::check::Zod::Literal(|| Value::from(true))),
                _ => None,
            },
        }
    }
}

/// No refinement of its own; checks every nested field that has one.
impl Check for DaemonErrorResponse {
    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "id" => crate::types::check::leaf(rest, crate::types::check::Zod::Union),
                "ok" => crate::types::check::leaf(rest, crate::types::check::Zod::Literal(|| Value::from(false))),
                "error" => <String as Check>::describe(rest),
                "code" => <Option<DaemonErrorCode> as Check>::describe(rest),
                "kind" => <Option<MachineErrorKind> as Check>::describe(rest),
                "status" => <Option<i64> as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

/// No refinement of its own; checks every nested field that has one.
impl Check for DaemonResponse {
    fn check(&self) -> Result<(), SchemaError> {
        match self {
            DaemonResponse::V0(v) => v.check(),
            DaemonResponse::V1(v) => v.check(),
        }
    }

    fn describe(path: &[Value]) -> Option<crate::types::check::Zod> {
        if path.is_empty() {
            return Some(crate::types::check::Zod::Union);
        }
        <DaemonOkResponse as Check>::describe(path).or_else(|| <DaemonErrorResponse as Check>::describe(path))
    }
}

/// Refinements zod puts on this shape, each an issue when it fails:
/// - `type`: literal "sys.sample"
impl Check for SysSample {
    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "cpu" => <JsNumber as Check>::describe(rest),
                "load1" => <JsNumber as Check>::describe(rest),
                "mem" => <SysSampleMem as Check>::describe(rest),
                "disk" => <SysSampleDisk as Check>::describe(rest),
                "at" => <JsNumber as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

/// No refinement of its own; checks every nested field that has one.
impl Check for SysSampleMem {
    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "used" => <JsNumber as Check>::describe(rest),
                "total" => <JsNumber as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

/// No refinement of its own; checks every nested field that has one.
impl Check for SysSampleDisk {
    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "used" => <JsNumber as Check>::describe(rest),
                "total" => <JsNumber as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

/// No refinement of its own; checks every nested field that has one.
impl Check for ReadingsAnswer {
    fn check(&self) -> Result<(), SchemaError> {
        self.points.check().map_err(|e| e.under("points"))?;
        Ok(())
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "points" => <Vec<SysPoint> as Check>::describe(rest),
                "stepMs" => <JsNumber as Check>::describe(rest),
                "from" => <JsNumber as Check>::describe(rest),
                "to" => <JsNumber as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

/// Refinements zod puts on this shape, each an issue when it fails:
/// - `type`: literal "workspace.sys"
impl Check for WorkspaceSysEvent {
    fn check(&self) -> Result<(), SchemaError> {
        literal_word(&self.r#type, "workspace.sys", "type")?;
        self.sample.check().map_err(|e| e.under("sample"))?;
        Ok(())
    }

    fn describe(path: &[Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "type" => Some(crate::types::check::Zod::Literal(|| Value::from("workspace.sys"))).filter(|_| rest.is_empty()),
                "workspaceId" => <String as Check>::describe(rest),
                "sample" => <SysSampleFrame as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

/// Refinements zod puts on this shape, each an issue when it fails:
/// - `type`: literal "place.sys"
impl Check for PlaceSysEvent {
    fn check(&self) -> Result<(), SchemaError> {
        literal_word(&self.r#type, "place.sys", "type")?;
        self.sample.check().map_err(|e| e.under("sample"))?;
        Ok(())
    }

    fn describe(path: &[Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "type" => Some(crate::types::check::Zod::Literal(|| Value::from("place.sys"))).filter(|_| rest.is_empty()),
                "placeId" => <String as Check>::describe(rest),
                "sample" => <SysSampleFrame as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

/// Refinements zod puts on this shape, each an issue when it fails:
/// - `placedBy`: catch null: a value that does not parse reads as absent
impl Check for ProcContainer {
    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "id" => <String as Check>::describe(rest),
                "name" => <String as Check>::describe(rest),
                "image" => <String as Check>::describe(rest),
                "state" => <String as Check>::describe(rest),
                "startedAt" => <Option<JsNumber> as Check>::describe(rest),
                "thread" => <Option<String> as Check>::describe(rest),
                "folder" => <Option<String> as Check>::describe(rest),
                "project" => <Option<String> as Check>::describe(rest),
                "memory" => <Option<JsNumber> as Check>::describe(rest),
                "cpu" => <Option<JsNumber> as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

/// Refinements zod puts on this shape, each an issue when it fails:
/// - `type`: literal "proc.snapshot"
impl Check for ProcSnapshot {
    fn check(&self) -> Result<(), SchemaError> {
        self.procs.check().map_err(|e| e.under("procs"))?;
        self.containers.check().map_err(|e| e.under("containers"))?;
        Ok(())
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "at" => <JsNumber as Check>::describe(rest),
                "daemon" => <i64 as Check>::describe(rest),
                "total" => <i64 as Check>::describe(rest),
                "procs" => <Vec<ProcEntry> as Check>::describe(rest),
                "seq" => <Option<i64> as Check>::describe(rest),
                "cost" => <Option<ProcCost> as Check>::describe(rest),
                "containers" => <Option<Vec<ProcContainer>> as Check>::describe(rest),
                "cgroups" => <Option<Vec<CgroupMemory>> as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

/// Refinements zod puts on this shape, each an issue when it fails:
/// - `type`: literal "proc.changes"
impl Check for ProcChanges {
    fn check(&self) -> Result<(), SchemaError> {
        self.procs.check().map_err(|e| e.under("procs"))?;
        self.containers.check().map_err(|e| e.under("containers"))?;
        Ok(())
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "at" => <JsNumber as Check>::describe(rest),
                "daemon" => <i64 as Check>::describe(rest),
                "total" => <i64 as Check>::describe(rest),
                "procs" => <Vec<ProcEntry> as Check>::describe(rest),
                "gone" => <Vec<i64> as Check>::describe(rest),
                "seq" => <i64 as Check>::describe(rest),
                "base" => <i64 as Check>::describe(rest),
                "cost" => <Option<ProcCost> as Check>::describe(rest),
                "containers" => <Option<Vec<ProcContainer>> as Check>::describe(rest),
                "cgroups" => <Option<Vec<CgroupMemory>> as Check>::describe(rest),
                _ => None,
            },
        }
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
    format!("{place} holds no copy of {image}; a fork there builds a copy first only of your image's current version")
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
    SaidFix {
        said: format!("{place} holds work no remote has, which a remove would lose: {}", unsaved.join("; ")),
        fix: format!("Keep a fork's work with wsp export or by pushing its branch, and copy a project folder's off {place} from the path named; then remove {place} again, or wsp remove {place} --force removes it anyway."),
    }
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
    SaidFix {
        said: format!("{sentence}, and its forks and projects go over its link"),
        fix: format!("Turn {place} on and remove it again once it answers; if it never will, wsp remove {place} --forget takes it, its forks, projects and threads out of this wsp, with nothing done on it beyond a try at wsp's own leave over ssh."),
    }
}
// machine-link.ts:314
// /** What a forget of a computer is refused with while its link answers: a remove takes wsp off it. */
// export const placeForgetAnswersRefusal = (place: string): { said: string; fix: string } => ({
//   said: `${place} is answering, so there is nothing to forget`,
//   fix: `wsp remove ${place} takes it out and takes wsp off it.`,
// });
pub fn place_forget_answers_refusal(place: &str) -> SaidFix {
    SaidFix {
        said: format!("{place} is answering, so there is nothing to forget"),
        fix: format!("wsp remove {place} takes it out and takes wsp off it."),
    }
}
// machine-link.ts:320
// /** What a remove that stopped part way through the forks and projects on a computer says: why, and which of them had
//  * already gone, since those are not coming back. */
// export const placeDropStoppedLine = (why: string, went: readonly string[]): string => (went.length === 0 ? why : `${why}; ${went.join(", ")} had already gone with the remove`);
pub fn place_drop_stopped_line(why: &str, went: &[String]) -> String {
    if went.is_empty() {
        return why.to_string();
    }
    format!("{why}; {} had already gone with the remove", went.join(", "))
}
// machine-link.ts:324
// /** What a remove of a project is refused with while its checkout on a computer of the person's holds work no remote
//  * has, named by its path there. */
// export const projectUnsavedRefusal = (project: string, unsaved: string): { said: string; fix: string } => ({
//   said: `${unsaved}, which removing ${project} would lose`,
//   fix: `Copy that work off its computer from the path named or push its branches, then remove ${project} again; wsp projects remove ${project} --force removes it anyway.`,
// });
pub fn project_unsaved_refusal(project: &str, unsaved: &str) -> SaidFix {
    SaidFix {
        said: format!("{unsaved}, which removing {project} would lose"),
        fix: format!("Copy that work off its computer from the path named or push its branches, then remove {project} again; wsp projects remove {project} --force removes it anyway."),
    }
}
// machine-link.ts:331
// /** How an unsaved read names a project folder: by its project and the checkout's path on the computer holding it,
//  * which is where a person goes to keep what is there. */
// export const projectFolderNamed = (project: string, checkout: string): string => `${project} at ${checkout}`;
pub fn project_folder_named(project: &str, checkout: &str) -> String {
    format!("{project} at {checkout}")
}
// machine-link.ts:363
// /** What a fork aimed at a place this host no longer holds a record for is refused with. Every computer on the
//  * list forks, so the only way to reach this is a record that went between the word being read and the fork being
//  * asked for: a remove, or a store another process wrote. */
// export const placeForksNowhereLine = (place: string): string => `${place} is no longer a place in this wsp, so nothing forks there`;
pub fn place_forks_nowhere_line(place: &str) -> String {
    format!("{place} is no longer a place in this wsp, so nothing forks there")
}
// machine-link.ts:368
// /** What a verb aimed at the bare computer a place is, rather than at a workspace forked on it, is refused with. A
//  * place holds its facts and its forks; the forks are the workspaces, so the refusal names the computer and the one
//  * road to a workspace there. */
// export const placeNotAWorkspaceLine = (place: string): string => `${place} is a computer you joined, not a workspace; its forks are the workspaces`;
pub fn place_not_a_workspace_line(place: &str) -> String {
    format!("{place} is a computer you joined, not a workspace; its forks are the workspaces")
}
// machine-link.ts:372
// export const placeNotAWorkspaceFix = (place: string): string => `Name a machine on it: wsp threads lists what runs on ${place}.`;
pub fn place_not_a_workspace_fix(place: &str) -> String {
    format!("Name a machine on it: wsp threads lists what runs on {place}.")
}
// machine-link.ts:374
// /** What a caller asking for the road to a workspace's own daemon is told, where that workspace runs none: a
//  * workspace on a computer somebody owns is that computer's directories under the computer's own daemon, and that
//  * daemon answers its files and its git through the host. Said rather than a route minted to a port nothing listens
//  * on, which is what the panes and the relay read before. */
// export const placeServesDaemonLine = (workspace: string, computer: string): string =>
//   `${workspace} has no daemon of its own: ${computer} answers its files and git through this host`;
pub fn place_serves_daemon_line(workspace: &str, computer: &str) -> String {
    format!("{workspace} has no daemon of its own: {computer} answers its files and git through this host")
}
// machine-link.ts:381
// /** What a watch of the ports or of the load is refused with on a workspace whose computer answers its daemon
//  * frames: both readings are that whole computer's, and one workspace reading them would read another workspace's
//  * listeners and load as its own. A pane asks for both on every link it opens and takes a refusal of either. */
// export const placeWatchesItselfLine = (computer: string): string =>
//   `${computer} watches its own ports and load, which are that computer's rather than one workspace's`;
pub fn place_watches_itself_line(computer: &str) -> String {
    format!("{computer} watches its own ports and load, which are that computer's rather than one workspace's")
}
// machine-link.ts:387
// /** What a watch, read or signal of processes is refused with on a workspace whose computer answers its daemon
//  * frames: that daemon acts on any pid on the computer, the computer's own and every other workspace's, so none of
//  * them goes up its link from one workspace's channel. The Processes pane shows it in place of the table. */
// export const forkProcsUnreadLine = (workspace: string, computer: string): string =>
//   `${workspace}'s processes on ${computer} are not readable from here yet`;
pub fn fork_procs_unread_line(workspace: &str, computer: &str) -> String {
    format!("{workspace}'s processes on {computer} are not readable from here yet")
}
// machine-link.ts:393
// /** What every other op is refused with on that workspace's channel: the frames that computer's daemon answers
//  * inside the workspace they name are its shells, its files and its git, and every other op there runs on the
//  * computer itself. */
// export const forkOpRefusedLine = (op: string, workspace: string, computer: string): string =>
//   `${computer} answers ${workspace}'s shells, files and git from here, not ${op}`;
pub fn fork_op_refused_line(op: &str, workspace: &str, computer: &str) -> String {
    format!("{computer} answers {workspace}'s shells, files and git from here, not {op}")
}
// machine-link.ts:402
// export const localRunsOneFix = (workspace: string): string => `Use ${workspace}, or name a computer that forks: wsp computers.`;
pub fn local_runs_one_fix(workspace: &str) -> String {
    format!("Use {workspace}, or name a computer that forks: wsp computers.")
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
    if at.platform != "linux" {
        return Some("wsp runs workspaces on a Linux computer".into());
    }
    let Some(controllers) = (at.read)(CGROUP_CONTROLLERS_PATH) else {
        return Some("this computer mounts cgroup v1 at /sys/fs/cgroup, and wsp runs workspaces on cgroup v2 alone: boot it with systemd.unified_cgroup_hierarchy=1".into());
    };
    let has: Vec<&str> = controllers.split_whitespace().collect();
    for wanted in ["memory", "cpu"] {
        if !has.contains(&wanted) {
            return Some(format!("this computer's cgroup root offers no {wanted} controller, which wsp needs to run workspaces here"));
        }
    }
    if let Some(overlay) = overlay_missing(at) {
        return Some(overlay);
    }
    if at.euid != Some(0) {
        return Some("wsp runs workspaces on this computer as root, and this daemon is not root".into());
    }
    None
}

fn overlay_missing(at: &KernelAt<'_>) -> Option<String> {
    let listed = || (at.read)(PROC_FILESYSTEMS_PATH).is_some_and(|text| text.split_whitespace().any(|fs| fs == "overlay"));
    if listed() {
        return None;
    }
    match (at.modprobe)(&["overlay"]) {
        ModprobeRun::Failed { error } => Some(overlay_wont_load(&format!("{MODPROBE_PATH} could not run: {error}"))),
        ModprobeRun::Ran { ok, stderr } => {
            if ok && listed() {
                return None;
            }
            match (at.modprobe)(&["-n", "overlay"]) {
                ModprobeRun::Ran { ok: true, .. } => {}
                _ => return Some("this computer's kernel has no overlay filesystem and no overlay module to load, which a workspace here reads this computer's own directories through".into()),
            }
            let last = stderr.trim_end().split('\n').next_back().unwrap_or_default().trim();
            let said = if last.is_empty() { "modprobe loaded it and the kernel still lists no overlay" } else { last };
            Some(overlay_wont_load(said))
        }
    }
}

fn overlay_wont_load(said: &str) -> String {
    format!("this computer's kernel has an overlay module that would not load ({said}), and a workspace here reads this computer's own directories through an overlay")
}
// machine-link.ts:453
// /** What a join of a computer whose kernel cannot boot the image is refused with, in the one sentence the daemon's
//  * own doctor named the reason in. A computer that cannot boot your image is not a place, so the join stops here
//  * and nothing is written on it. */
// export const placeCannotBootLine = (place: string, reason?: string): string =>
//   reason === undefined ? `${place} cannot run wsp workspaces, so it cannot be a place` : `${place} cannot run wsp workspaces: ${reason.replace(/this computer('s)?/g, (_, owns) => (owns === undefined ? "it" : "its"))}`;
pub fn place_cannot_boot_line(place: &str, reason: Option<&str>) -> String {
    match reason {
        None => format!("{place} cannot run wsp workspaces, so it cannot be a place"),
        Some(reason) => {
            format!("{place} cannot run wsp workspaces: {}", reason.replace("this computer's", "its").replace("this computer", "it"))
        }
    }
}
// machine-link.ts:459
// /** The same sentence off a computer's own report, or nothing while it runs workspaces: one reading for the join's
//  * refusal, the row, and every act that runs inside a copy there. */
// export const placeBlocked = (place: string, report: Pick<PlaceReport, "runsWorkspaces" | "workspacesBlocked">): string | undefined =>
//   report.runsWorkspaces ? undefined : placeCannotBootLine(place, report.workspacesBlocked);
pub fn place_blocked(place: &str, runs_workspaces: bool, workspaces_blocked: Option<&str>) -> Option<String> {
    if runs_workspaces {
        return None;
    }
    Some(place_cannot_boot_line(place, workspaces_blocked))
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
    if wsp_door != Some(false) {
        return None;
    }
    let why = wsp_door_blocked.unwrap_or("its daemon opened no door for them");
    let fix = match daemon_unit {
        None => format!("remove {place} and add it again"),
        Some(unit) => format!("restart wsp's daemon there with systemctl restart {unit}"),
    };
    Some(format!("threads on {place} get no wsp tools: {why}; {fix}"))
}
// machine-link.ts:482
// /** What a word that names no place this host holds is refused with, naming the ones it does. */
// export const noSuchPlaceRefusal = (word: string, held: readonly string[]): string => `no place named ${word}; you have ${held.join(", ")}`;
pub fn no_such_place_refusal(word: &str, held: &[String]) -> String {
    format!("no place named {word}; you have {}", held.join(", "))
}
// machine-link.ts:485
// /** Whether a row is a computer somebody joined to this wsp: a computer, and not the one the host runs on, whose
//  * files and threads are that host's own. The one reading, so the road that runs on the link, the road at the
//  * terminal and the list of places a fork could stand on cannot disagree about which rows are those computers. */
// export const isJoinedComputer = (place: { id: string; kind: string }): boolean => place.kind === "computer" && place.id !== HERE_PLACE_ID;
pub fn is_joined_computer(id: &str, kind: &str) -> bool {
    kind == "computer" && id != "here"
}
// machine-link.ts:490
// /** What a build of the image at a place that cannot take one is refused with: a copy needs a builder forked there
//  * and that builder's disk copied, and a computer somebody joined does neither. */
// export const placeBuildsNoImageLine = (place: string): string =>
//   `${place} takes no copy of your image: a copy is built by forking a machine there and copying its disk, and ${place} does neither`;
pub fn place_builds_no_image_line(place: &str) -> String {
    format!(
        "{place} takes no copy of your image: a copy is built by forking a machine there and copying its disk, and {place} does neither"
    )
}
// machine-link.ts:499
// /** What a vault archive is refused with: where the reading stopped, why, and what that refusal did, which is not
//  * the same on the two roads. The archive is refused whole, since a builder that wrote one member nobody asked for
//  * wrote every other member too. */
// export const vaultMemberRefusal = (road: VaultRoad, member: string, why: string): string =>
//   `the image's sign-in archive is refused at ${member}: ${why}; ${road === "seal" ? "the seal is refused and no version is recorded" : "nothing of it was imported"}`;
pub fn vault_member_refusal(road: VaultRoad, member: &str, why: &str) -> String {
    let refused = match road {
        VaultRoad::Seal => "the seal is refused and no version is recorded",
        VaultRoad::Import => "nothing of it was imported",
    };
    format!("the image's sign-in archive is refused at {member}: {why}; {refused}")
}
// machine-link.ts:505
// /** What a copy is refused with for a record whose vault kept no path list: sealed before the record held which
//  * paths its sign-ins live at, so no other place can tell a member the seal asked for from one it did not. */
// export const vaultUnlistedRefusal = (name: string, version: number): string =>
//   `${name} v${version} was sealed before its record kept which paths its sign-ins live at, so no other place can check its archive against them; cut the next version`;
pub fn vault_unlisted_refusal(name: &str, version: i64) -> String {
    format!("{name} v{version} was sealed before its record kept which paths its sign-ins live at, so no other place can check its archive against them; cut the next version")
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
    if snapshot.seq != Some(changes.base) {
        return None;
    }
    let mut rows: std::collections::BTreeMap<i64, ProcEntry> = snapshot.procs.iter().map(|p| (p.pid, p.clone())).collect();
    for pid in &changes.gone {
        rows.remove(pid);
    }
    for p in &changes.procs {
        rows.insert(p.pid, p.clone());
    }
    Some(ProcSnapshot {
        at: changes.at.clone(),
        daemon: changes.daemon,
        total: changes.total,
        procs: rows.into_values().collect(),
        seq: Some(changes.seq),
        cost: changes.cost.clone(),
        containers: changes.containers.clone().or_else(|| snapshot.containers.clone()),
        cgroups: changes.cgroups.clone(),
    })
}

/// Reads one machine-link request as zod's discriminated union on `op` reads it, so a refusal's first issue is zod's: a value
/// that is not an object is one invalid_type issue at the root (expected "object"); an absent or unknown `op` is
/// one invalid_union_discriminator issue on ["op"] whose detail `options` lists every op of MachineLinkRequest in order
/// and whose message is "Invalid discriminator value. Expected 'a' | 'b' | ..."; otherwise the op's own fields are read
/// one op at a time, so the path of a refusal inside them survives (serde loses it inside a tagged enum), then the
/// variant's checks. Unknown keys are dropped.
pub fn parse_machine_link_request(frame: &Value) -> Result<MachineLinkRequest, SchemaError> {
    if !frame.is_object() {
        return crate::types::check::parse::<MachineLinkRequest>(frame);
    }
    let op = match frame.get("op").and_then(Value::as_str) {
        Some(op) if MACHINE_LINK_OPS.contains(&op) => op,
        _ => return Err(op_discriminator("op", MACHINE_LINK_OPS)),
    };
    let request = match op {
        "machine.backend" => MachineLinkRequest::MachineBackend { id: parse::<IdOnly>(frame)?.id },
        "machine.capacity" => MachineLinkRequest::MachineCapacity { id: parse::<IdOnly>(frame)?.id },
        "machine.checkKey" => MachineLinkRequest::MachineCheckKey { id: parse::<IdOnly>(frame)?.id },
        "machine.create" => {
            let p = parse::<CreateFrame>(frame)?;
            MachineLinkRequest::MachineCreate { id: p.id, spec: p.spec }
        }
        "machine.get" => {
            let p = parse::<MachineFrame>(frame)?;
            MachineLinkRequest::MachineGet { id: p.id, machine_id: p.machine_id }
        }
        "machine.list" => {
            let p = parse::<ListFrame>(frame)?;
            MachineLinkRequest::MachineList { id: p.id, labels: p.labels }
        }
        "machine.exec" => {
            let p = parse::<ExecFrame>(frame)?;
            MachineLinkRequest::MachineExec { id: p.id, machine_id: p.machine_id, cmd: p.cmd, timeout_ms: p.timeout_ms, stdin: p.stdin }
        }
        "machine.pause" | "machine.resume" | "machine.kill" | "machine.state" | "machine.describe" | "machine.facts"
        | "machine.metrics" => {
            let p = parse::<MachineFrame>(frame)?;
            let (id, machine_id) = (p.id, p.machine_id);
            match op {
                "machine.pause" => MachineLinkRequest::MachinePause { id, machine_id },
                "machine.resume" => MachineLinkRequest::MachineResume { id, machine_id },
                "machine.kill" => MachineLinkRequest::MachineKill { id, machine_id },
                "machine.state" => MachineLinkRequest::MachineState { id, machine_id },
                "machine.describe" => MachineLinkRequest::MachineDescribe { id, machine_id },
                "machine.facts" => MachineLinkRequest::MachineFacts { id, machine_id },
                _ => MachineLinkRequest::MachineMetrics { id, machine_id },
            }
        }
        "machine.daemonAnswers" => {
            let p = parse::<DaemonAnswersFrame>(frame)?;
            MachineLinkRequest::MachineDaemonAnswers { id: p.id, machine_id: p.machine_id, timeout_ms: p.timeout_ms }
        }
        "machine.previewUrl" => {
            let p = parse::<PreviewFrame>(frame)?;
            MachineLinkRequest::MachinePreviewUrl { id: p.id, machine_id: p.machine_id, port: p.port }
        }
        "machine.downloadUrl" | "machine.uploadUrl" => {
            let p = parse::<PathFrame>(frame)?;
            let (id, machine_id, path) = (p.id, p.machine_id, p.path);
            if op == "machine.downloadUrl" {
                MachineLinkRequest::MachineDownloadUrl { id, machine_id, path }
            } else {
                MachineLinkRequest::MachineUploadUrl { id, machine_id, path }
            }
        }
        _ => {
            let p = parse::<PutBytesFrame>(frame)?;
            MachineLinkRequest::MachinePutBytes {
                id: p.id,
                machine_id: p.machine_id,
                path: p.path,
                upload_id: p.upload_id,
                seq: p.seq,
                last: p.last,
                data: p.data,
                timeout_ms: p.timeout_ms,
            }
        }
    };
    request.check()?;
    Ok(request)
}

/// The fields every machine-link op's frame carries, read on their own (see parse_machine_link_request).
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct IdOnly {
    id: ReqId,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct MachineFrame {
    id: ReqId,
    machine_id: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CreateFrame {
    id: ReqId,
    spec: MachineSpec,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ListFrame {
    id: ReqId,
    #[serde(default)]
    labels: Option<IndexMap<String, String>>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ExecFrame {
    id: ReqId,
    machine_id: String,
    cmd: String,
    #[serde(default)]
    timeout_ms: Option<i64>,
    #[serde(default)]
    stdin: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct DaemonAnswersFrame {
    id: ReqId,
    machine_id: String,
    #[serde(default)]
    timeout_ms: Option<i64>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct PreviewFrame {
    id: ReqId,
    machine_id: String,
    port: i64,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct PathFrame {
    id: ReqId,
    machine_id: String,
    path: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct PutBytesFrame {
    id: ReqId,
    machine_id: String,
    path: String,
    upload_id: String,
    seq: i64,
    last: bool,
    data: String,
    #[serde(default)]
    timeout_ms: Option<i64>,
}

/// The field words a machine-link frame's keys are read by, whichever op carries them.
fn request_field(path: &[Value]) -> Option<crate::types::check::Zod> {
    match path.split_first()? {
        (key, rest) => match key.as_str()? {
            "id" => crate::types::check::leaf(rest, crate::types::check::Zod::Union),
            "machineId" | "path" | "uploadId" | "cmd" | "stdin" | "data" => <String as Check>::describe(rest),
            "timeoutMs" | "seq" | "port" => <i64 as Check>::describe(rest),
            "last" => <bool as Check>::describe(rest),
            "spec" => <MachineSpec as Check>::describe(rest),
            "labels" => <Option<IndexMap<String, String>> as Check>::describe(rest),
            _ => None,
        },
    }
}

macro_rules! frame_describe {
    ($($name:ident),*) => {$(
        impl Check for $name {
            fn describe(path: &[Value]) -> Option<crate::types::check::Zod> {
                if path.is_empty() {
                    return Some(crate::types::check::Zod::Word("object"));
                }
                request_field(path)
            }
        }
    )*};
}
frame_describe!(IdOnly, MachineFrame, CreateFrame, ListFrame, ExecFrame, DaemonAnswersFrame, PreviewFrame, PathFrame, PutBytesFrame);

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
impl Check for ProcCost {}
impl Check for SysHistoryReply {}
impl Check for SysPoint {}
impl Check for TranscriptsListReply {}
impl Check for TranscriptsReadReply {}
impl Check for UsageLogsReply {}

// A word list zod reads with z.enum: serde refuses any other word, so nothing is left to check.
impl Check for DaemonSupervisor {
    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        crate::types::check::leaf(path, crate::types::check::Zod::Options(&["systemd", "entrypoint"]))
    }
}
impl Check for MachineSpecOnIdle {
    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        crate::types::check::leaf(path, crate::types::check::Zod::Options(&["pause", "kill"]))
    }
}
impl Check for TemplateRowStatus {
    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        crate::types::check::leaf(path, crate::types::check::Zod::Options(&["building", "ready", "failed"]))
    }
}
impl Check for MachineErrorKind {
    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        crate::types::check::leaf(
            path,
            crate::types::check::Zod::Options(&[
                "concurrency",
                "plan",
                "missing",
                "conflict",
                "snapshotUnavailable",
                "transient",
                "auth",
                "unknown",
                "absent",
            ]),
        )
    }
}
impl Check for DaemonErrorCode {
    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        crate::types::check::leaf(
            path,
            crate::types::check::Zod::Options(&[
                "unsupported",
                "outside-root",
                "not-found",
                "not-a-directory",
                "not-a-file",
                "not-a-git-repo",
                "bad-request",
                "forbidden",
                "no-host-cli",
                "no-git-credential",
                "rate-limited",
                "on-default-branch",
            ]),
        )
    }
}

/// machine.backend answers the backend's facts and machine.capacity its room: the same shapes, by the same name
/// the TS gives them (machine-link.ts:245-246).
pub type MachineBackendReply = BackendFacts;
pub type MachineCapacityReply = crate::wire::capabilities::PlaceCapacity;
