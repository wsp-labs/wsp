// SPDX-License-Identifier: AGPL-3.0-only
use super::*;

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
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::types::kept::record_opt",
        serialize_with = "crate::types::check::js_record_opt"
    )]
    pub envs: Option<IndexMap<String, String>>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::types::kept::record_opt",
        serialize_with = "crate::types::check::js_record_opt"
    )]
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
    pub exit_code: JsInt,
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
    pub used_bytes: Option<JsInt>,
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
    pub wake_attempts: JsInt,
    pub daemon_answers_ms: JsNumber,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub resume_asks: Option<LifecycleBudgetsResumeAsks>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MachineListRow {
    pub id: String,
    pub state: MachineState,
    #[serde(serialize_with = "crate::types::check::js_record", deserialize_with = "crate::types::kept::record")]
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
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::types::kept::record_opt",
        serialize_with = "crate::types::check::js_record_opt"
    )]
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
        timeout_ms: Option<JsInt>,
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
        timeout_ms: Option<JsInt>,
    },
    #[serde(rename = "machine.previewUrl", rename_all = "camelCase")]
    MachinePreviewUrl { id: ReqId, machine_id: String, port: JsInt },
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
        seq: JsInt,
        last: bool,
        data: String,
        #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
        timeout_ms: Option<JsInt>,
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
    pub mem_bytes: Option<JsInt>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub cpu_usage_usec: Option<JsInt>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub uptime_ms: Option<JsInt>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub procs: Option<JsInt>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub quiet_for_ms: Option<JsInt>,
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

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DaemonOkResponse {
    #[serde(deserialize_with = "crate::types::check::nullable")]
    pub id: Option<ReqId>,
    pub ok: True,
    /// zod passthrough: unknown keys kept, in the order read.
    #[serde(flatten, deserialize_with = "crate::types::kept::record")]
    pub rest: IndexMap<String, Value>,
}
/// JavaScript writes the keys that read as array indexes first, ahead of `id` and `ok`.
impl Serialize for DaemonOkResponse {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        let mut entries = vec![
            ("id".to_string(), serde_json::to_value(&self.id).map_err(serde::ser::Error::custom)?),
            ("ok".to_string(), Value::Bool(true)),
        ];
        entries.extend(self.rest.iter().map(|(k, v)| (k.clone(), v.clone())));
        crate::types::kept::serialize_object(&entries, s)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DaemonErrorResponse {
    #[serde(deserialize_with = "crate::types::check::nullable")]
    pub id: Option<ReqId>,
    pub ok: False,
    pub error: String,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub code: Option<DaemonErrorCode>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub kind: Option<MachineErrorKind>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub status: Option<JsInt>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(untagged)]
pub enum DaemonResponse {
    V0(DaemonOkResponse),
    V1(DaemonErrorResponse),
}

crate::zod_union!(DaemonResponse { V0(DaemonOkResponse), V1(DaemonErrorResponse) }

);

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
    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = match obj.get("type") {
            None => vec![crate::types::check::required::<SysSampleType>("type")],
            Some(tag) => crate::types::check::issues_of::<SysSampleType>(tag).into_iter().map(|e| e.under("type")).collect(),
        };
        let mut body = obj.clone();
        body.shift_remove("type");
        out.extend(crate::types::check::issues_of::<SysSample>(&serde_json::Value::Object(body)));
        Some(out)
    }
}

crate::literal!(WorkspaceSysType, "workspace.sys");

crate::literal!(PlaceSysType, "place.sys");

crate::literal!(SysSampleType, "sys.sample");

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceSysEvent {
    pub r#type: WorkspaceSysType,
    pub workspace_id: String,
    pub sample: SysSampleFrame,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaceSysEvent {
    pub r#type: PlaceSysType,
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
    pub daemon: JsInt,
    pub total: JsInt,
    pub procs: Vec<ProcEntry>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub seq: Option<JsInt>,
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
    pub daemon: JsInt,
    pub total: JsInt,
    pub procs: Vec<ProcEntry>,
    pub gone: Vec<JsInt>,
    pub seq: JsInt,
    pub base: JsInt,
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
        self.copy.check().map_err(|e| e.under("copy"))?;
        self.shares.check().map_err(|e| e.under("shares"))?;
        self.binds.check().map_err(|e| e.under("binds"))?;
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        crate::types::keys::req_key::<MachineKind>(obj, "kind", &mut out);
        crate::types::keys::opt_key::<String>(obj, "template", &mut out);
        crate::types::keys::opt_key::<String>(obj, "fromSnapshot", &mut out);
        crate::types::keys::opt_key::<JsNumber>(obj, "cpu", &mut out);
        crate::types::keys::opt_key::<JsNumber>(obj, "memMb", &mut out);
        crate::types::keys::opt_key::<JsNumber>(obj, "diskGb", &mut out);
        crate::types::keys::opt_key::<IndexMap<String, String>>(obj, "envs", &mut out);
        crate::types::keys::opt_key::<IndexMap<String, String>>(obj, "labels", &mut out);
        crate::types::keys::opt_key::<MachineSpecOnIdle>(obj, "onIdle", &mut out);
        crate::types::keys::opt_key::<JsNumber>(obj, "idleTimeoutMs", &mut out);
        crate::types::keys::opt_key::<String>(obj, "idempotencyKey", &mut out);
        crate::types::keys::opt_key::<bool>(obj, "engine", &mut out);
        crate::types::keys::opt_key::<WorkspaceCopy>(obj, "copy", &mut out);
        crate::types::keys::opt_key::<Vec<MachineShare>>(obj, "shares", &mut out);
        crate::types::keys::opt_key::<Vec<MachineBind>>(obj, "binds", &mut out);
        Some(out)
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
    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        crate::types::keys::req_key::<JsInt>(obj, "exitCode", &mut out);
        crate::types::keys::req_key::<String>(obj, "stdout", &mut out);
        crate::types::keys::req_key::<String>(obj, "stderr", &mut out);
        Some(out)
    }

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
            num_at_least(v.as_f64(), 0.0).map_err(|e| e.under("usedBytes"))?;
        }
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        crate::types::keys::opt_key::<JsNumber>(obj, "cpu", &mut out);
        crate::types::keys::opt_key::<JsNumber>(obj, "memMb", &mut out);
        crate::types::keys::opt_key::<JsNumber>(obj, "diskGb", &mut out);
        crate::types::keys::opt_key::<String>(obj, "createdAt", &mut out);
        crate::types::keys::opt_key_with::<JsInt>(obj, "usedBytes", &mut out, Self::rule_used_bytes);
        Some(out)
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
impl MachineShape {
    fn rule_used_bytes(x: &Option<JsInt>) -> Result<(), crate::types::check::SchemaError> {
        if let Some(v) = *x {
            num_at_least(v.as_f64(), 0.0)?;
        }
        Ok(())
    }
}

/// No refinement of its own; checks every nested field that has one.
impl Check for MachineLife {
    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        crate::types::keys::req_key::<bool>(obj, "firstLife", &mut out);
        Some(out)
    }

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
    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        crate::types::keys::req_key::<String>(obj, "url", &mut out);
        crate::types::keys::req_key::<String>(obj, "token", &mut out);
        crate::types::keys::req_key::<JsNumber>(obj, "expiresAt", &mut out);
        Some(out)
    }

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
    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        crate::types::keys::req_key::<String>(obj, "id", &mut out);
        crate::types::keys::opt_key::<String>(obj, "name", &mut out);
        crate::types::keys::req_key::<JsNumber>(obj, "sizeBytes", &mut out);
        crate::types::keys::opt_key::<JsNumber>(obj, "restoredBytes", &mut out);
        crate::types::keys::opt_key::<String>(obj, "createdAt", &mut out);
        crate::types::keys::dbl_key::<String>(obj, "parent", &mut out);
        Some(out)
    }

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
    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        crate::types::keys::req_key::<String>(obj, "id", &mut out);
        crate::types::keys::req_key::<String>(obj, "name", &mut out);
        crate::types::keys::req_key::<TemplateRowStatus>(obj, "status", &mut out);
        crate::types::keys::opt_key::<String>(obj, "error", &mut out);
        crate::types::keys::opt_key::<String>(obj, "createdAt", &mut out);
        Some(out)
    }

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
    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        crate::types::keys::req_key::<JsNumber>(obj, "freeGb", &mut out);
        crate::types::keys::req_key::<JsNumber>(obj, "usdPerGbMonth", &mut out);
        crate::types::keys::req_key::<String>(obj, "billedFrom", &mut out);
        Some(out)
    }

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
        num_at_least(self.wake_attempts.as_f64(), 1.0).map_err(|e| e.under("wakeAttempts"))?;
        num_above(self.daemon_answers_ms.0, 0.0).map_err(|e| e.under("daemonAnswersMs"))?;
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        crate::types::keys::req_key_with::<JsInt>(obj, "wakeAttempts", &mut out, Self::rule_wake_attempts);
        crate::types::keys::req_key_with::<JsNumber>(obj, "daemonAnswersMs", &mut out, Self::rule_daemon_answers_ms);
        crate::types::keys::opt_key::<LifecycleBudgetsResumeAsks>(obj, "resumeAsks", &mut out);
        Some(out)
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
impl LifecycleBudgets {
    fn rule_wake_attempts(x: &JsInt) -> Result<(), crate::types::check::SchemaError> {
        num_at_least((*x).as_f64(), 1.0)?;
        Ok(())
    }
    fn rule_daemon_answers_ms(x: &JsNumber) -> Result<(), crate::types::check::SchemaError> {
        num_above(x.0, 0.0)?;
        Ok(())
    }
}

/// No refinement of its own; checks every nested field that has one.
impl Check for LifecycleBudgetsResumeAsks {
    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        crate::types::keys::req_key::<JsNumber>(obj, "everyMs", &mut out);
        crate::types::keys::req_key::<JsNumber>(obj, "forMs", &mut out);
        Some(out)
    }

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
    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        crate::types::keys::req_key::<String>(obj, "id", &mut out);
        crate::types::keys::req_key::<MachineState>(obj, "state", &mut out);
        crate::types::keys::req_key::<IndexMap<String, String>>(obj, "labels", &mut out);
        crate::types::keys::opt_key::<WorkspaceSize>(obj, "size", &mut out);
        Some(out)
    }

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

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        crate::types::keys::req_key::<String>(obj, "offer", &mut out);
        crate::types::keys::req_key::<Capabilities>(obj, "capabilities", &mut out);
        crate::types::keys::req_key::<BackendFactsPricing>(obj, "pricing", &mut out);
        crate::types::keys::opt_key::<BackendFactsLifecycle>(obj, "lifecycle", &mut out);
        crate::types::keys::opt_key::<BackendFactsBaseTemplates>(obj, "baseTemplates", &mut out);
        crate::types::keys::opt_key_with::<String>(obj, "logins", &mut out, Self::rule_logins);
        crate::types::keys::opt_key_with::<String>(obj, "projects", &mut out, Self::rule_projects);
        Some(out)
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
impl BackendFacts {
    fn rule_logins(x: &Option<String>) -> Result<(), crate::types::check::SchemaError> {
        if let Some(logins) = x {
            text_at_least(logins, 1)?;
            refine(crate::wire::limits::is_plain_path(logins), "an absolute path on the computer")?;
        }
        Ok(())
    }
    fn rule_projects(x: &Option<String>) -> Result<(), crate::types::check::SchemaError> {
        if let Some(projects) = x {
            text_at_least(projects, 1)?;
            refine(crate::wire::limits::is_plain_path(projects), "an absolute path on the computer")?;
        }
        Ok(())
    }
}

/// No refinement of its own; checks every nested field that has one.
impl Check for BackendFactsPricing {
    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        crate::types::keys::req_key::<WorkspaceSize>(obj, "defaultSize", &mut out);
        crate::types::keys::req_key::<SnapshotStoragePricing>(obj, "snapshotStorage", &mut out);
        crate::types::keys::opt_key::<JsNumber>(obj, "builderDiskGb", &mut out);
        Some(out)
    }

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

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        crate::types::keys::req_key::<LifecycleBudgets>(obj, "budgets", &mut out);
        Some(out)
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
    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        crate::types::keys::req_key::<String>(obj, "sandbox", &mut out);
        crate::types::keys::req_key::<String>(obj, "desktop", &mut out);
        Some(out)
    }

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
