// SPDX-License-Identifier: AGPL-3.0-only
//! Port of packages/protocol/src/views/place.ts at a3b870a6e: its zod shapes and plain constants, written by the generator.

use serde::{Deserialize, Serialize};

use crate::types::agents_report::AgentSignInState;
use crate::types::check::{Check, SchemaError};
use crate::types::js_number::JsNumber;
use crate::types::recipe_file::RecipeFile;
use crate::views::place_kind::PlaceKind;
use crate::views::workspace::ProjectAddStage;
use crate::views::workspace::ProjectView;
use crate::views::workspace::WorkspaceAgents;
use crate::wire::capabilities::MachineSizeOffer;
use crate::wire::limits::PortForward;
use indexmap::IndexMap;
use crate::wire::limits::RelayPort;
use wsp_frames::True;
use crate::wire::capabilities::WorkspaceSize;

/// TS: packages/protocol/src/views/place.ts:27 at a3b870a6e.
/// A Record in the TS: its keys in the TS's order.
pub const PLACE_ADD_WORDS: &[(&str, &str)] = &[
    ("connect", "connecting over ssh"),
    ("host-key", "remembering the box's host key in ~/.ssh/known_hosts"),
    ("check", "checking it can run wsp"),
    ("chip", "reading its chip and system"),
    ("root", "checking it logs in as root"),
    ("system", "checking for systemd and cgroup v2"),
    ("disk", "checking it has room for the base tools"),
    ("reach", "checking it can reach this computer"),
    ("wsp", "installing wsp"),
    ("service", "starting the daemon"),
    ("join", "waiting for it to connect to this computer"),
];

/// TS: packages/protocol/src/views/place.ts:60 at a3b870a6e.
pub const PLACE_LOGIN_REFUSED_KIND: &str = "login";

/// TS: packages/protocol/src/views/place.ts:65 at a3b870a6e.
pub const PLACE_HOST_KEY_KIND: &str = "host-key";

/// TS: packages/protocol/src/views/place.ts:73 at a3b870a6e.
pub const PLACE_SUDO_KIND: &str = "sudo";

/// TS: packages/protocol/src/views/place.ts:218 at a3b870a6e.
/// A Record in the TS: its keys in the TS's order.
pub const PROVISION_KIND_WORDS: &[(&str, &str)] = &[("tool", "tool"), ("file", "file"), ("server", "MCP server")];

/// TS: packages/protocol/src/views/place.ts:252 at a3b870a6e.
pub const GITHUB_ROW: &str = "github";

/// TS: packages/protocol/src/views/place.ts:253 at a3b870a6e.
pub const GITHUB_CLI: &str = "gh";

/// TS: packages/protocol/src/views/place.ts:283 at a3b870a6e.
/// A Record in the TS: its keys in the TS's order.
pub const SETUP_STEP_CLASS: &[(&str, &str)] = &[
    ("floor", "blocks"),
    ("agents", "blocks-all"),
    ("signins", "needs-you"),
    ("clis", "fixable"),
    ("skills", "fixable"),
    ("mcp", "fixable"),
    ("plugins", "fixable"),
    ("configs", "fixable"),
    ("folders", "important"),
    ("folderServers", "fixable"),
    ("github", "important"),
    ("context", "fixable"),
];

/// TS: packages/protocol/src/views/place.ts:301 at a3b870a6e.
/// A Record in the TS: its keys in the TS's order.
pub const SETUP_STEP_WORDS: &[(&str, &str)] = &[
    ("floor", "the base tools"),
    ("agents", "the agents"),
    ("signins", "the agents' sign-ins"),
    ("clis", "the CLIs"),
    ("skills", "the skills"),
    ("mcp", "the agents' own files and MCP servers"),
    ("plugins", "the plugins"),
    ("configs", "git and the shell"),
    ("folders", "the folders, as projects"),
    ("folderServers", "the projects' own MCP servers"),
    ("github", "the GitHub sign-in"),
    ("context", "what the agents read about this computer"),
];

/// TS: packages/protocol/src/views/place.ts:15 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ForwardOpenEvent {
    pub forward: PortForward,
}

impl ForwardOpenEvent {
    /// The `type` key's value: the enum holding this struct writes and reads it as its tag.
    pub const TYPE: &'static str = "forward.open";
}

impl Check for ForwardOpenEvent {
    /// Nested checks: forward.
    fn check(&self) -> Result<(), SchemaError> {
        self.forward.check().map_err(|e| e.under("forward"))?;
        Ok(())
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "forward" => <PortForward as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

/// TS: packages/protocol/src/views/place.ts:16 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ForwardCloseEvent {
    pub workspace_id: String,
    pub port: RelayPort,
}

impl ForwardCloseEvent {
    /// The `type` key's value: the enum holding this struct writes and reads it as its tag.
    pub const TYPE: &'static str = "forward.close";
}

impl Check for ForwardCloseEvent {
    /// Nested checks: port.
    fn check(&self) -> Result<(), SchemaError> {
        self.port.check().map_err(|e| e.under("port"))?;
        Ok(())
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "workspaceId" => <String as Check>::describe(rest),
                "port" => <RelayPort as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

/// TS: packages/protocol/src/views/place.ts:22 at a3b870a6e.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PlaceAddStep {
    #[serde(rename = "connect")]
    Connect,
    #[serde(rename = "host-key")]
    HostKey,
    #[serde(rename = "check")]
    Check,
    #[serde(rename = "chip")]
    Chip,
    #[serde(rename = "root")]
    Root,
    #[serde(rename = "system")]
    System,
    #[serde(rename = "disk")]
    Disk,
    #[serde(rename = "reach")]
    Reach,
    #[serde(rename = "wsp")]
    Wsp,
    #[serde(rename = "service")]
    Service,
    #[serde(rename = "join")]
    Join,
}

impl Check for PlaceAddStep {
    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        crate::types::check::leaf(path, crate::types::check::Zod::Options(&["connect", "host-key", "check", "chip", "root", "system", "disk", "reach", "wsp", "service", "join"]))
    }
}

/// TS: packages/protocol/src/views/place.ts:69 at a3b870a6e.
/// TS: views/place.ts:69 at a3b870a6e..
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct SudoPassword(pub String);

impl Check for SudoPassword {
    fn check(&self) -> Result<(), SchemaError> {
        crate::types::check::text_at_most(&self.0, 1024)?;
        crate::types::check::regex_check(crate::types::check::no_line_breaks(&self.0))
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        crate::types::check::leaf(path, crate::types::check::Zod::Word("string"))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PlaceStageEventState {
    #[serde(rename = "running")]
    Running,
    #[serde(rename = "done")]
    Done,
    #[serde(rename = "failed")]
    Failed,
}

impl Check for PlaceStageEventState {
    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        crate::types::check::leaf(path, crate::types::check::Zod::Options(&["running", "done", "failed"]))
    }
}

/// TS: packages/protocol/src/views/place.ts:77 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaceStageEvent {
    pub add_id: String,
    pub step: PlaceAddStep,
    pub state: PlaceStageEventState,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub note: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub place_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub ms: Option<i64>,
}

impl PlaceStageEvent {
    /// The `type` key's value: the enum holding this struct writes and reads it as its tag.
    pub const TYPE: &'static str = "place.stage";
}

impl Check for PlaceStageEvent {
    /// Rules: ms: >= 0.
    /// Nested checks: step.
    fn check(&self) -> Result<(), SchemaError> {
        self.step.check().map_err(|e| e.under("step"))?;
        self.state.check().map_err(|e| e.under("state"))?;
        if let Some(v) = self.ms {
            crate::types::check::num_at_least(v as f64, 0.0).map_err(|e| e.under("ms"))?;
        }
        Ok(())
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "addId" => <String as Check>::describe(rest),
                "step" => <PlaceAddStep as Check>::describe(rest),
                "state" => <PlaceStageEventState as Check>::describe(rest),
                "note" => <Option<String> as Check>::describe(rest),
                "placeId" => <Option<String> as Check>::describe(rest),
                "ms" => <Option<i64> as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PlaceAddJobState {
    #[serde(rename = "running")]
    Running,
    #[serde(rename = "done")]
    Done,
    #[serde(rename = "failed")]
    Failed,
}

impl Check for PlaceAddJobState {
    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        crate::types::check::leaf(path, crate::types::check::Zod::Options(&["running", "done", "failed"]))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PlaceAddJobStepsItemState {
    #[serde(rename = "running")]
    Running,
    #[serde(rename = "done")]
    Done,
    #[serde(rename = "failed")]
    Failed,
}

impl Check for PlaceAddJobStepsItemState {
    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        crate::types::check::leaf(path, crate::types::check::Zod::Options(&["running", "done", "failed"]))
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaceAddJobStepsItem {
    pub step: PlaceAddStep,
    pub state: PlaceAddJobStepsItemState,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub note: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub ms: Option<i64>,
}

impl Check for PlaceAddJobStepsItem {
    /// Rules: ms: >= 0.
    /// Nested checks: step.
    fn check(&self) -> Result<(), SchemaError> {
        self.step.check().map_err(|e| e.under("step"))?;
        self.state.check().map_err(|e| e.under("state"))?;
        if let Some(v) = self.ms {
            crate::types::check::num_at_least(v as f64, 0.0).map_err(|e| e.under("ms"))?;
        }
        Ok(())
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "step" => <PlaceAddStep as Check>::describe(rest),
                "state" => <PlaceAddJobStepsItemState as Check>::describe(rest),
                "note" => <Option<String> as Check>::describe(rest),
                "ms" => <Option<i64> as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

/// TS: packages/protocol/src/views/place.ts:94 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaceAddJob {
    pub add_id: String,
    pub address: String,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub ssh_port: Option<i64>,
    pub started_at: String,
    pub state: PlaceAddJobState,
    pub steps: Vec<PlaceAddJobStepsItem>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub said: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub fix: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub kind: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub place_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub host_key: Option<String>,
}

impl Check for PlaceAddJob {
    /// Nested checks: steps.
    fn check(&self) -> Result<(), SchemaError> {
        self.state.check().map_err(|e| e.under("state"))?;
        self.steps.check().map_err(|e| e.under("steps"))?;
        Ok(())
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "addId" => <String as Check>::describe(rest),
                "address" => <String as Check>::describe(rest),
                "sshPort" => <Option<i64> as Check>::describe(rest),
                "startedAt" => <String as Check>::describe(rest),
                "state" => <PlaceAddJobState as Check>::describe(rest),
                "steps" => <Vec<PlaceAddJobStepsItem> as Check>::describe(rest),
                "said" => <Option<String> as Check>::describe(rest),
                "fix" => <Option<String> as Check>::describe(rest),
                "kind" => <Option<String> as Check>::describe(rest),
                "placeId" => <Option<String> as Check>::describe(rest),
                "hostKey" => <Option<String> as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum DoctorLineEventStream {
    #[serde(rename = "out")]
    Out,
    #[serde(rename = "err")]
    Err,
}

impl Check for DoctorLineEventStream {
    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        crate::types::check::leaf(path, crate::types::check::Zod::Options(&["out", "err"]))
    }
}

/// TS: packages/protocol/src/views/place.ts:127 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DoctorLineEvent {
    pub doctor_id: String,
    pub line: String,
    pub stream: DoctorLineEventStream,
}

impl DoctorLineEvent {
    /// The `type` key's value: the enum holding this struct writes and reads it as its tag.
    pub const TYPE: &'static str = "doctor.line";
}

impl Check for DoctorLineEvent {
    fn check(&self) -> Result<(), SchemaError> {
        self.stream.check().map_err(|e| e.under("stream"))?;
        Ok(())
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "doctorId" => <String as Check>::describe(rest),
                "line" => <String as Check>::describe(rest),
                "stream" => <DoctorLineEventStream as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

/// TS: packages/protocol/src/views/place.ts:148 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaceBack {
    pub box_port: i64,
}

impl Check for PlaceBack {
    /// Rules: boxPort: >= 1; boxPort: <= 65535.
    fn check(&self) -> Result<(), SchemaError> {
        crate::types::check::num_at_least(self.box_port as f64, 1.0).map_err(|e| e.under("boxPort"))?;
        crate::types::check::num_at_most(self.box_port as f64, 65535.0).map_err(|e| e.under("boxPort"))?;
        Ok(())
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "boxPort" => <i64 as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

/// TS: packages/protocol/src/views/place.ts:157 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaceRoad {
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub ssh: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub from: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub back: Option<PlaceBack>,
}

impl Check for PlaceRoad {
    /// Rules: ssh: at most 300 UTF-16 units; from: at most 300 UTF-16 units.
    /// Nested checks: back.
    fn check(&self) -> Result<(), SchemaError> {
        if let Some(v) = &self.ssh {
            crate::types::check::text_at_most(v, 300).map_err(|e| e.under("ssh"))?;
        }
        if let Some(v) = &self.from {
            crate::types::check::text_at_most(v, 300).map_err(|e| e.under("from"))?;
        }
        self.back.check().map_err(|e| e.under("back"))?;
        Ok(())
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "ssh" => <Option<String> as Check>::describe(rest),
                "from" => <Option<String> as Check>::describe(rest),
                "back" => <Option<PlaceBack> as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

/// TS: packages/protocol/src/views/place.ts:167 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaceDialled {
    pub at: String,
    pub answered: bool,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub round_trip_ms: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub said: Option<String>,
}

impl Check for PlaceDialled {
    /// Rules: roundTripMs: >= 0; said: at most 2000 UTF-16 units.
    fn check(&self) -> Result<(), SchemaError> {
        if let Some(v) = self.round_trip_ms {
            crate::types::check::num_at_least(v as f64, 0.0).map_err(|e| e.under("roundTripMs"))?;
        }
        if let Some(v) = &self.said {
            crate::types::check::text_at_most(v, 2000).map_err(|e| e.under("said"))?;
        }
        Ok(())
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "at" => <String as Check>::describe(rest),
                "answered" => <bool as Check>::describe(rest),
                "roundTripMs" => <Option<i64> as Check>::describe(rest),
                "said" => <Option<String> as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

/// TS: packages/protocol/src/views/place.ts:180 at a3b870a6e.
/// The project a workspace on a computer you own is made with: a checkout copied once for this workspace, and where it is mounted inside.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceCopy {
    pub from: String,
    pub at: String,
}

impl Check for WorkspaceCopy {
    fn check(&self) -> Result<(), SchemaError> {
        crate::types::check::text_at_least(&self.from, 1).map_err(|e| e.under("from"))?;
        crate::types::check::refine(crate::wire::limits::is_plain_path(&self.from), "an absolute path on the computer").map_err(|e| e.under("from"))?;
        crate::types::check::text_at_least(&self.at, 1).map_err(|e| e.under("at"))?;
        crate::types::check::refine(crate::wire::limits::is_plain_path(&self.at), "an absolute path inside the workspace").map_err(|e| e.under("at"))?;
        Ok(())
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "from" => <String as Check>::describe(rest),
                "at" => <String as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

/// TS: packages/protocol/src/views/place.ts:187 at a3b870a6e.
/// The daemon's own enum: wsp-frames declares it, with the same values in the same order.
pub use wsp_frames::CopyWord;

impl Check for CopyWord {
    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        crate::types::check::leaf(path, crate::types::check::Zod::Options(&["reflink", "snapshot", "plain"]))
    }
}

/// TS: packages/protocol/src/views/place.ts:194 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MachineShare {
    pub source: String,
    pub target: String,
}

impl Check for MachineShare {
    /// Rules: source: at least 1 UTF-16 units; source: a refine (spec/shapes.md rule 7); target: at least 1 UTF-16 units; target: a refine (spec/shapes.md rule 7).
    fn check(&self) -> Result<(), SchemaError> {
        crate::types::check::text_at_least(&self.source, 1).map_err(|e| e.under("source"))?;
        crate::types::check::refine(crate::wire::limits::is_plain_path(&self.source), "an absolute path on the computer").map_err(|e| e.under("source"))?;
        crate::types::check::text_at_least(&self.target, 1).map_err(|e| e.under("target"))?;
        crate::types::check::refine(crate::wire::limits::is_plain_path(&self.target), "an absolute path inside the workspace").map_err(|e| e.under("target"))?;
        Ok(())
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "source" => <String as Check>::describe(rest),
                "target" => <String as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

/// TS: packages/protocol/src/views/place.ts:204 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MachineBind {
    pub source: String,
    pub target: String,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub read_only: Option<bool>,
}

impl Check for MachineBind {
    /// Rules: source: at least 1 UTF-16 units; source: a refine (spec/shapes.md rule 7); target: at least 1 UTF-16 units; target: a refine (spec/shapes.md rule 7).
    fn check(&self) -> Result<(), SchemaError> {
        crate::types::check::text_at_least(&self.source, 1).map_err(|e| e.under("source"))?;
        crate::types::check::refine(crate::wire::limits::is_plain_path(&self.source), "an absolute path on the computer").map_err(|e| e.under("source"))?;
        crate::types::check::text_at_least(&self.target, 1).map_err(|e| e.under("target"))?;
        crate::types::check::refine(crate::wire::limits::is_plain_path(&self.target), "an absolute path inside the workspace").map_err(|e| e.under("target"))?;
        Ok(())
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "source" => <String as Check>::describe(rest),
                "target" => <String as Check>::describe(rest),
                "readOnly" => <Option<bool> as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

/// TS: packages/protocol/src/views/place.ts:213 at a3b870a6e.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PlaceProvisionKind {
    #[serde(rename = "tool")]
    Tool,
    #[serde(rename = "file")]
    File,
    #[serde(rename = "server")]
    Server,
}

impl Check for PlaceProvisionKind {
    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        crate::types::check::leaf(path, crate::types::check::Zod::Options(&["tool", "file", "server"]))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PlaceProvisionRowOutcome {
    #[serde(rename = "installed")]
    Installed,
    #[serde(rename = "present")]
    Present,
    #[serde(rename = "failed")]
    Failed,
    #[serde(rename = "skipped")]
    Skipped,
}

impl Check for PlaceProvisionRowOutcome {
    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        crate::types::check::leaf(path, crate::types::check::Zod::Options(&["installed", "present", "failed", "skipped"]))
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaceProvisionRowProject {
    pub id: String,
}

impl Check for PlaceProvisionRowProject {
    /// Rules: id: at least 1 UTF-16 units.
    fn check(&self) -> Result<(), SchemaError> {
        crate::types::check::text_at_least(&self.id, 1).map_err(|e| e.under("id"))?;
        Ok(())
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "id" => <String as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

/// TS: packages/protocol/src/views/place.ts:229 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaceProvisionRow {
    pub id: String,
    pub label: String,
    pub outcome: PlaceProvisionRowOutcome,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub kind: Option<PlaceProvisionKind>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub step: Option<PlaceSetupStep>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub note: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub fix: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub keys: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub ms: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub earlier: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub project: Option<PlaceProvisionRowProject>,
}

impl Check for PlaceProvisionRow {
    /// Rules: id: at least 1 UTF-16 units; label: at least 1 UTF-16 units; keys[]: at least 1 UTF-16 units; ms: >= 0.
    /// Nested checks: kind, step, project.
    fn check(&self) -> Result<(), SchemaError> {
        crate::types::check::text_at_least(&self.id, 1).map_err(|e| e.under("id"))?;
        crate::types::check::text_at_least(&self.label, 1).map_err(|e| e.under("label"))?;
        self.outcome.check().map_err(|e| e.under("outcome"))?;
        self.kind.check().map_err(|e| e.under("kind"))?;
        self.step.check().map_err(|e| e.under("step"))?;
        if let Some(list) = &self.keys {
            for (i, item) in list.iter().enumerate() {
                crate::types::check::text_at_least(item, 1).map_err(|e| e.under(i).under("keys"))?;
            }
        }
        if let Some(v) = self.ms {
            crate::types::check::num_at_least(v as f64, 0.0).map_err(|e| e.under("ms"))?;
        }
        self.project.check().map_err(|e| e.under("project"))?;
        Ok(())
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "id" => <String as Check>::describe(rest),
                "label" => <String as Check>::describe(rest),
                "outcome" => <PlaceProvisionRowOutcome as Check>::describe(rest),
                "kind" => <Option<PlaceProvisionKind> as Check>::describe(rest),
                "step" => <Option<PlaceSetupStep> as Check>::describe(rest),
                "note" => <Option<String> as Check>::describe(rest),
                "fix" => <Option<String> as Check>::describe(rest),
                "keys" => <Option<Vec<String>> as Check>::describe(rest),
                "ms" => <Option<i64> as Check>::describe(rest),
                "earlier" => <Option<bool> as Check>::describe(rest),
                "project" => <Option<PlaceProvisionRowProject> as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

/// TS: packages/protocol/src/views/place.ts:297 at a3b870a6e.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PlaceSetupStep {
    #[serde(rename = "floor")]
    Floor,
    #[serde(rename = "agents")]
    Agents,
    #[serde(rename = "signins")]
    Signins,
    #[serde(rename = "clis")]
    Clis,
    #[serde(rename = "skills")]
    Skills,
    #[serde(rename = "mcp")]
    Mcp,
    #[serde(rename = "plugins")]
    Plugins,
    #[serde(rename = "configs")]
    Configs,
    #[serde(rename = "folders")]
    Folders,
    #[serde(rename = "folderServers")]
    FolderServers,
    #[serde(rename = "github")]
    Github,
    #[serde(rename = "context")]
    Context,
}

impl Check for PlaceSetupStep {
    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        crate::types::check::leaf(path, crate::types::check::Zod::Options(&["floor", "agents", "signins", "clis", "skills", "mcp", "plugins", "configs", "folders", "folderServers", "github", "context"]))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PlaceSetupLineState {
    #[serde(rename = "running")]
    Running,
    #[serde(rename = "done")]
    Done,
    #[serde(rename = "failed")]
    Failed,
    #[serde(rename = "skipped")]
    Skipped,
}

impl Check for PlaceSetupLineState {
    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        crate::types::check::leaf(path, crate::types::check::Zod::Options(&["running", "done", "failed", "skipped"]))
    }
}

/// TS: packages/protocol/src/views/place.ts:317 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaceSetupLine {
    pub step: PlaceSetupStep,
    pub state: PlaceSetupLineState,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub note: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub ms: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub started_at: Option<String>,
}

impl Check for PlaceSetupLine {
    /// Rules: ms: >= 0.
    /// Nested checks: step.
    fn check(&self) -> Result<(), SchemaError> {
        self.step.check().map_err(|e| e.under("step"))?;
        self.state.check().map_err(|e| e.under("state"))?;
        if let Some(v) = self.ms {
            crate::types::check::num_at_least(v as f64, 0.0).map_err(|e| e.under("ms"))?;
        }
        Ok(())
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "step" => <PlaceSetupStep as Check>::describe(rest),
                "state" => <PlaceSetupLineState as Check>::describe(rest),
                "note" => <Option<String> as Check>::describe(rest),
                "ms" => <Option<i64> as Check>::describe(rest),
                "startedAt" => <Option<String> as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PlaceWaitState {
    #[serde(rename = "waiting")]
    Waiting,
    #[serde(rename = "expired")]
    Expired,
}

impl Check for PlaceWaitState {
    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        crate::types::check::leaf(path, crate::types::check::Zod::Options(&["waiting", "expired"]))
    }
}

/// TS: packages/protocol/src/views/place.ts:328 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaceWait {
    pub row: String,
    pub label: String,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub code: Option<String>,
    pub expires_at: String,
    pub state: PlaceWaitState,
}

impl Check for PlaceWait {
    fn check(&self) -> Result<(), SchemaError> {
        self.state.check().map_err(|e| e.under("state"))?;
        Ok(())
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "row" => <String as Check>::describe(rest),
                "label" => <String as Check>::describe(rest),
                "url" => <Option<String> as Check>::describe(rest),
                "code" => <Option<String> as Check>::describe(rest),
                "expiresAt" => <String as Check>::describe(rest),
                "state" => <PlaceWaitState as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PlaceSetupState {
    #[serde(rename = "running")]
    Running,
    #[serde(rename = "done")]
    Done,
    #[serde(rename = "failed")]
    Failed,
}

impl Check for PlaceSetupState {
    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        crate::types::check::leaf(path, crate::types::check::Zod::Options(&["running", "done", "failed"]))
    }
}

/// TS: packages/protocol/src/views/place.ts:341 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaceSetup {
    pub state: PlaceSetupState,
    pub add_id: String,
    pub started_at: String,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub finished_at: Option<String>,
    pub steps: Vec<PlaceSetupLine>,
    pub waiting: Vec<PlaceWait>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub said: Option<String>,
}

impl Check for PlaceSetup {
    /// Nested checks: steps, waiting.
    fn check(&self) -> Result<(), SchemaError> {
        self.state.check().map_err(|e| e.under("state"))?;
        self.steps.check().map_err(|e| e.under("steps"))?;
        self.waiting.check().map_err(|e| e.under("waiting"))?;
        Ok(())
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "state" => <PlaceSetupState as Check>::describe(rest),
                "addId" => <String as Check>::describe(rest),
                "startedAt" => <String as Check>::describe(rest),
                "finishedAt" => <Option<String> as Check>::describe(rest),
                "steps" => <Vec<PlaceSetupLine> as Check>::describe(rest),
                "waiting" => <Vec<PlaceWait> as Check>::describe(rest),
                "said" => <Option<String> as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

/// TS: packages/protocol/src/views/place.ts:355 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaceApplied {
    pub hash: String,
    pub at: String,
    pub rows: Vec<PlaceProvisionRow>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub items: Option<IndexMap<String, String>>,
}

impl Check for PlaceApplied {
    /// Nested checks: rows.
    fn check(&self) -> Result<(), SchemaError> {
        self.rows.check().map_err(|e| e.under("rows"))?;
        Ok(())
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "hash" => <String as Check>::describe(rest),
                "at" => <String as Check>::describe(rest),
                "rows" => <Vec<PlaceProvisionRow> as Check>::describe(rest),
                "items" => <Option<IndexMap<String, String>> as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PlaceSyncState {
    #[serde(rename = "behind")]
    Behind,
    #[serde(rename = "running")]
    Running,
}

impl Check for PlaceSyncState {
    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        crate::types::check::leaf(path, crate::types::check::Zod::Options(&["behind", "running"]))
    }
}

/// TS: packages/protocol/src/views/place.ts:368 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaceSync {
    pub state: PlaceSyncState,
    pub changes: Vec<String>,
    pub since: String,
}

impl Check for PlaceSync {
    fn check(&self) -> Result<(), SchemaError> {
        self.state.check().map_err(|e| e.under("state"))?;
        Ok(())
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "state" => <PlaceSyncState as Check>::describe(rest),
                "changes" => <Vec<String> as Check>::describe(rest),
                "since" => <String as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

/// TS: packages/protocol/src/views/place.ts:373 at a3b870a6e.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SetupEnd {
    #[serde(rename = "ready")]
    Ready,
    #[serde(rename = "needs-you")]
    NeedsYou,
    #[serde(rename = "failed")]
    Failed,
}

impl Check for SetupEnd {
    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        crate::types::check::leaf(path, crate::types::check::Zod::Options(&["ready", "needs-you", "failed"]))
    }
}

/// TS: packages/protocol/src/views/place.ts:379 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaceSetupEvent {
    pub add_id: String,
    pub place_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub line: Option<PlaceSetupLine>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub wait: Option<PlaceWait>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub landed: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub running: Option<Vec<PlaceSetupStep>>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub end: Option<SetupEnd>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub said: Option<String>,
}

impl PlaceSetupEvent {
    /// The `type` key's value: the enum holding this struct writes and reads it as its tag.
    pub const TYPE: &'static str = "place.setup";
}

impl Check for PlaceSetupEvent {
    /// Nested checks: line, wait, running, end.
    fn check(&self) -> Result<(), SchemaError> {
        self.line.check().map_err(|e| e.under("line"))?;
        self.wait.check().map_err(|e| e.under("wait"))?;
        self.running.check().map_err(|e| e.under("running"))?;
        self.end.check().map_err(|e| e.under("end"))?;
        Ok(())
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "addId" => <String as Check>::describe(rest),
                "placeId" => <String as Check>::describe(rest),
                "line" => <Option<PlaceSetupLine> as Check>::describe(rest),
                "wait" => <Option<PlaceWait> as Check>::describe(rest),
                "landed" => <Option<String> as Check>::describe(rest),
                "running" => <Option<Vec<PlaceSetupStep>> as Check>::describe(rest),
                "end" => <Option<SetupEnd> as Check>::describe(rest),
                "said" => <Option<String> as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

/// TS: packages/protocol/src/views/place.ts:397 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaceSyncEvent {
    pub place_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub sync: Option<PlaceSync>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub line: Option<PlaceSetupLine>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub applied: Option<PlaceApplied>,
}

impl PlaceSyncEvent {
    /// The `type` key's value: the enum holding this struct writes and reads it as its tag.
    pub const TYPE: &'static str = "place.sync";
}

impl Check for PlaceSyncEvent {
    /// Nested checks: sync, line, applied.
    fn check(&self) -> Result<(), SchemaError> {
        self.sync.check().map_err(|e| e.under("sync"))?;
        self.line.check().map_err(|e| e.under("line"))?;
        self.applied.check().map_err(|e| e.under("applied"))?;
        Ok(())
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "placeId" => <String as Check>::describe(rest),
                "sync" => <Option<PlaceSync> as Check>::describe(rest),
                "line" => <Option<PlaceSetupLine> as Check>::describe(rest),
                "applied" => <Option<PlaceApplied> as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

/// TS: packages/protocol/src/views/place.ts:402 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecipesChangedEvent {
    pub slug: String,
}

impl RecipesChangedEvent {
    /// The `type` key's value: the enum holding this struct writes and reads it as its tag.
    pub const TYPE: &'static str = "recipes.changed";
}

impl Check for RecipesChangedEvent {
    fn check(&self) -> Result<(), SchemaError> {
        Ok(())
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "slug" => <String as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

/// A zod union: the first variant that parses wins, in this order.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum AddLineStep {
    PlaceAddStep(PlaceAddStep),
    PlaceSetupStep(PlaceSetupStep),
}

impl Check for AddLineStep {
    fn check(&self) -> Result<(), SchemaError> {
        match self {
            AddLineStep::PlaceAddStep(v) => v.check(),
            AddLineStep::PlaceSetupStep(v) => v.check(),
        }
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        if path.is_empty() { return Some(crate::types::check::Zod::Union); }
        <PlaceAddStep as Check>::describe(path).or_else(|| <PlaceSetupStep as Check>::describe(path))
    }
}

/// TS: packages/protocol/src/views/place.ts:406 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AddLine {
    pub step: AddLineStep,
    pub state: PlaceSetupLineState,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub note: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub ms: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub started_at: Option<String>,
}

impl Check for AddLine {
    /// Rules: ms: >= 0.
    /// Nested checks: step.
    fn check(&self) -> Result<(), SchemaError> {
        self.step.check().map_err(|e| e.under("step"))?;
        self.state.check().map_err(|e| e.under("state"))?;
        if let Some(v) = self.ms {
            crate::types::check::num_at_least(v as f64, 0.0).map_err(|e| e.under("ms"))?;
        }
        Ok(())
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "step" => <AddLineStep as Check>::describe(rest),
                "state" => <PlaceSetupLineState as Check>::describe(rest),
                "note" => <Option<String> as Check>::describe(rest),
                "ms" => <Option<i64> as Check>::describe(rest),
                "startedAt" => <Option<String> as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

/// TS: packages/protocol/src/views/place.ts:411 at a3b870a6e.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PendingStep {
    #[serde(rename = "connect")]
    Connect,
    #[serde(rename = "check")]
    Check,
    #[serde(rename = "wsp")]
    Wsp,
    #[serde(rename = "floor")]
    Floor,
    #[serde(rename = "choosing")]
    Choosing,
}

impl Check for PendingStep {
    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        crate::types::check::leaf(path, crate::types::check::Zod::Options(&["connect", "check", "wsp", "floor", "choosing"]))
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PendingComputerFailed {
    pub said: String,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub fix: Option<String>,
}

impl Check for PendingComputerFailed {
    fn check(&self) -> Result<(), SchemaError> {
        Ok(())
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "said" => <String as Check>::describe(rest),
                "fix" => <Option<String> as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

/// TS: packages/protocol/src/views/place.ts:416 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PendingComputer {
    pub id: String,
    pub address: String,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub ssh_port: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub place_id: Option<String>,
    pub step: PendingStep,
    pub choices: RecipeFile,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub recipe: Option<String>,
    pub started_at: String,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub failed: Option<PendingComputerFailed>,
}

impl Check for PendingComputer {
    /// Nested checks: step, choices, failed.
    fn check(&self) -> Result<(), SchemaError> {
        self.step.check().map_err(|e| e.under("step"))?;
        self.choices.check().map_err(|e| e.under("choices"))?;
        self.failed.check().map_err(|e| e.under("failed"))?;
        Ok(())
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "id" => <String as Check>::describe(rest),
                "address" => <String as Check>::describe(rest),
                "sshPort" => <Option<i64> as Check>::describe(rest),
                "name" => <Option<String> as Check>::describe(rest),
                "placeId" => <Option<String> as Check>::describe(rest),
                "step" => <PendingStep as Check>::describe(rest),
                "choices" => <RecipeFile as Check>::describe(rest),
                "recipe" => <Option<String> as Check>::describe(rest),
                "startedAt" => <String as Check>::describe(rest),
                "failed" => <Option<PendingComputerFailed> as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

pub use super::place_more::*;

// ---- Hand-written below: the functions of the unit.

// views/place.ts:114 at a3b870a6e
// export function withPlaceStage(job: PlaceAddJob, e: Pick<PlaceStageEvent, "step" | "state" | "note" | "placeId" | "ms">): PlaceAddJob {
//   const line = { step: e.step, state: e.state, ...(e.note !== undefined ? { note: e.note } : {}), ...(e.ms !== undefined ? { ms: e.ms } : {}) };
//   const at = job.steps.findIndex(s => s.step === e.step);
//   const steps = at === -1 ? [...job.steps, line] : job.steps.map((s, i) => (i === at ? line : s));
//   if (e.step === "join" && e.state === "done") return { ...job, steps, state: "done", ...(e.placeId !== undefined ? { placeId: e.placeId } : {}) };
//   if (e.state === "failed") return { ...job, steps, state: "failed", ...(job.said === undefined && e.note !== undefined ? { said: e.note } : {}) };
//   return { ...job, steps };
// }
/// The job with one stage line folded in: the step's line replaced where the job has one (in place), else appended;
/// a done `join` ends the job done (taking the event's place id where it has one); a failed line ends it failed, its
/// note becoming the job's `said` only where the job has none yet.
pub fn with_place_stage(job: &PlaceAddJob, e: &PlaceStageEvent) -> PlaceAddJob {
    todo!()
}

// views/place.ts:254 at a3b870a6e (private)
// const SIGN_IN_ROW = "signins/";
const SIGN_IN_ROW: &str = "signins/";

// views/place.ts:256 at a3b870a6e
// export const signInRowId = (agent: string): string => `${SIGN_IN_ROW}${agent}`;
pub fn sign_in_row_id(agent: &str) -> String {
    todo!()
}

// views/place.ts:258 at a3b870a6e
// export const signInOfRow = (id: string): string | undefined => (id.startsWith(SIGN_IN_ROW) ? id.slice(SIGN_IN_ROW.length) : id === GITHUB_ROW ? GITHUB_CLI : undefined);
pub fn sign_in_of_row(id: &str) -> Option<String> {
    todo!()
}

// views/place.ts:261 at a3b870a6e
// export function provisionCounts(rows: readonly PlaceProvisionRow[]): Partial<Record<PlaceProvisionKind, number>> {
//   const counts: Partial<Record<PlaceProvisionKind, number>> = {};
//   for (const r of rows) {
//     const kind = r.kind ?? "tool";
//     counts[kind] = (counts[kind] ?? 0) + 1;
//   }
//   return counts;
// }
/// How many rows of each kind (an absent kind counts as `tool`), keys in the order each kind is first met.
pub fn provision_counts(rows: &[PlaceProvisionRow]) -> indexmap::IndexMap<PlaceProvisionKind, i64> {
    todo!()
}

// views/place.ts:502 at a3b870a6e
// export function macKindOf(said: string): MacKind | undefined {
//   const word = said.replace(/\s+/g, "").toLowerCase();
//   if (word.startsWith("macbook")) return "macbook";
//   if (word.startsWith("imac")) return "imac";
//   if (word.startsWith("macmini")) return "mac-mini";
//   if (word.startsWith("macstudio")) return "mac-studio";
//   if (word.startsWith("macpro")) return "mac-pro";
//   return undefined;
// }
/// Read with every whitespace character taken out and lower-cased.
pub fn mac_kind_of(said: &str) -> Option<MacKind> {
    todo!()
}

// views/place.ts:462 at a3b870a6e
// export interface PlaceSetAlso {
//   name?: string | undefined;
//   ssh?: string | undefined;
//   recipe?: string | undefined;
// }
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaceSetAlso {
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub ssh: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub recipe: Option<String>,
}

// views/place.ts:702 at a3b870a6e
// export const placeForgetsOnly = (holds: PlaceHolds): boolean => holds.away === true && (holds.forks.length > 0 || holds.projects.length > 0);
pub fn place_forgets_only(holds: &PlaceHolds) -> bool {
    todo!()
}
