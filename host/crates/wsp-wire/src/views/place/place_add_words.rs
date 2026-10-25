// SPDX-License-Identifier: AGPL-3.0-only
use super::*;

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

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        crate::types::keys::req_key::<PortForward>(obj, "forward", &mut out);
        Some(out)
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

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        crate::types::keys::req_key::<String>(obj, "workspaceId", &mut out);
        crate::types::keys::req_key::<RelayPort>(obj, "port", &mut out);
        Some(out)
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
        crate::types::check::leaf(
            path,
            crate::types::check::Zod::Options(&[
                "connect", "host-key", "check", "chip", "root", "system", "disk", "reach", "wsp", "service", "join",
            ]),
        )
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
    pub ms: Option<JsInt>,
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
            crate::types::check::num_at_least(v.0 .0, 0.0).map_err(|e| e.under("ms"))?;
        }
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        crate::types::keys::req_key::<String>(obj, "addId", &mut out);
        crate::types::keys::req_key::<PlaceAddStep>(obj, "step", &mut out);
        crate::types::keys::req_key::<PlaceStageEventState>(obj, "state", &mut out);
        crate::types::keys::opt_key::<String>(obj, "note", &mut out);
        crate::types::keys::opt_key::<String>(obj, "placeId", &mut out);
        crate::types::keys::opt_key_with::<JsInt>(obj, "ms", &mut out, Self::rule_ms);
        Some(out)
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
impl PlaceStageEvent {
    fn rule_ms(x: &Option<JsInt>) -> Result<(), crate::types::check::SchemaError> {
        if let Some(v) = *x {
            crate::types::check::num_at_least(v.0 .0, 0.0)?;
        }
        Ok(())
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
    pub ms: Option<JsInt>,
}
impl Check for PlaceAddJobStepsItem {
    /// Rules: ms: >= 0.
    /// Nested checks: step.
    fn check(&self) -> Result<(), SchemaError> {
        self.step.check().map_err(|e| e.under("step"))?;
        self.state.check().map_err(|e| e.under("state"))?;
        if let Some(v) = self.ms {
            crate::types::check::num_at_least(v.0 .0, 0.0).map_err(|e| e.under("ms"))?;
        }
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        crate::types::keys::req_key::<PlaceAddStep>(obj, "step", &mut out);
        crate::types::keys::req_key::<PlaceAddJobStepsItemState>(obj, "state", &mut out);
        crate::types::keys::opt_key::<String>(obj, "note", &mut out);
        crate::types::keys::opt_key_with::<JsInt>(obj, "ms", &mut out, Self::rule_ms);
        Some(out)
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
impl PlaceAddJobStepsItem {
    fn rule_ms(x: &Option<JsInt>) -> Result<(), crate::types::check::SchemaError> {
        if let Some(v) = *x {
            crate::types::check::num_at_least(v.0 .0, 0.0)?;
        }
        Ok(())
    }
}

/// TS: packages/protocol/src/views/place.ts:94 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaceAddJob {
    pub add_id: String,
    pub address: String,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub ssh_port: Option<JsInt>,
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

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        crate::types::keys::req_key::<String>(obj, "addId", &mut out);
        crate::types::keys::req_key::<String>(obj, "address", &mut out);
        crate::types::keys::opt_key::<JsInt>(obj, "sshPort", &mut out);
        crate::types::keys::req_key::<String>(obj, "startedAt", &mut out);
        crate::types::keys::req_key::<PlaceAddJobState>(obj, "state", &mut out);
        crate::types::keys::req_key::<Vec<PlaceAddJobStepsItem>>(obj, "steps", &mut out);
        crate::types::keys::opt_key::<String>(obj, "said", &mut out);
        crate::types::keys::opt_key::<String>(obj, "fix", &mut out);
        crate::types::keys::opt_key::<String>(obj, "kind", &mut out);
        crate::types::keys::opt_key::<String>(obj, "placeId", &mut out);
        crate::types::keys::opt_key::<String>(obj, "hostKey", &mut out);
        Some(out)
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

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        crate::types::keys::req_key::<String>(obj, "doctorId", &mut out);
        crate::types::keys::req_key::<String>(obj, "line", &mut out);
        crate::types::keys::req_key::<DoctorLineEventStream>(obj, "stream", &mut out);
        Some(out)
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
    pub box_port: JsInt,
}
impl Check for PlaceBack {
    /// Rules: boxPort: >= 1; boxPort: <= 65535.
    fn check(&self) -> Result<(), SchemaError> {
        crate::types::check::num_at_least(self.box_port.0 .0, 1.0).map_err(|e| e.under("boxPort"))?;
        crate::types::check::num_at_most(self.box_port.0 .0, 65535.0).map_err(|e| e.under("boxPort"))?;
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        crate::types::keys::req_key_with::<JsInt>(obj, "boxPort", &mut out, Self::rule_box_port);
        Some(out)
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
impl PlaceBack {
    fn rule_box_port(x: &JsInt) -> Result<(), crate::types::check::SchemaError> {
        crate::types::check::num_at_least(x.0 .0, 1.0)?;
        crate::types::check::num_at_most(x.0 .0, 65535.0)?;
        Ok(())
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

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        crate::types::keys::opt_key_with::<String>(obj, "ssh", &mut out, Self::rule_ssh);
        crate::types::keys::opt_key_with::<String>(obj, "from", &mut out, Self::rule_from);
        crate::types::keys::opt_key::<PlaceBack>(obj, "back", &mut out);
        Some(out)
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
impl PlaceRoad {
    fn rule_ssh(x: &Option<String>) -> Result<(), crate::types::check::SchemaError> {
        if let Some(v) = x {
            crate::types::check::text_at_most(v, 300)?;
        }
        Ok(())
    }
    fn rule_from(x: &Option<String>) -> Result<(), crate::types::check::SchemaError> {
        if let Some(v) = x {
            crate::types::check::text_at_most(v, 300)?;
        }
        Ok(())
    }
}

/// TS: packages/protocol/src/views/place.ts:167 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaceDialled {
    pub at: String,
    pub answered: bool,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub round_trip_ms: Option<JsInt>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub said: Option<String>,
}
impl Check for PlaceDialled {
    /// Rules: roundTripMs: >= 0; said: at most 2000 UTF-16 units.
    fn check(&self) -> Result<(), SchemaError> {
        if let Some(v) = self.round_trip_ms {
            crate::types::check::num_at_least(v.0 .0, 0.0).map_err(|e| e.under("roundTripMs"))?;
        }
        if let Some(v) = &self.said {
            crate::types::check::text_at_most(v, 2000).map_err(|e| e.under("said"))?;
        }
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        crate::types::keys::req_key::<String>(obj, "at", &mut out);
        crate::types::keys::req_key::<bool>(obj, "answered", &mut out);
        crate::types::keys::opt_key_with::<JsInt>(obj, "roundTripMs", &mut out, Self::rule_round_trip_ms);
        crate::types::keys::opt_key_with::<String>(obj, "said", &mut out, Self::rule_said);
        Some(out)
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
impl PlaceDialled {
    fn rule_round_trip_ms(x: &Option<JsInt>) -> Result<(), crate::types::check::SchemaError> {
        if let Some(v) = *x {
            crate::types::check::num_at_least(v.0 .0, 0.0)?;
        }
        Ok(())
    }
    fn rule_said(x: &Option<String>) -> Result<(), crate::types::check::SchemaError> {
        if let Some(v) = x {
            crate::types::check::text_at_most(v, 2000)?;
        }
        Ok(())
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
        crate::types::check::refine(crate::wire::limits::is_plain_path(&self.from), "an absolute path on the computer")
            .map_err(|e| e.under("from"))?;
        crate::types::check::text_at_least(&self.at, 1).map_err(|e| e.under("at"))?;
        crate::types::check::refine(crate::wire::limits::is_plain_path(&self.at), "an absolute path inside the workspace")
            .map_err(|e| e.under("at"))?;
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        crate::types::keys::req_key_with::<String>(obj, "from", &mut out, Self::rule_from);
        crate::types::keys::req_key_with::<String>(obj, "at", &mut out, Self::rule_at);
        Some(out)
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
impl WorkspaceCopy {
    fn rule_from(x: &String) -> Result<(), crate::types::check::SchemaError> {
        crate::types::check::text_at_least(x, 1)?;
        crate::types::check::refine(crate::wire::limits::is_plain_path(x), "an absolute path on the computer")?;
        Ok(())
    }
    fn rule_at(x: &String) -> Result<(), crate::types::check::SchemaError> {
        crate::types::check::text_at_least(x, 1)?;
        crate::types::check::refine(crate::wire::limits::is_plain_path(x), "an absolute path inside the workspace")?;
        Ok(())
    }
}

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
        crate::types::check::refine(crate::wire::limits::is_plain_path(&self.source), "an absolute path on the computer")
            .map_err(|e| e.under("source"))?;
        crate::types::check::text_at_least(&self.target, 1).map_err(|e| e.under("target"))?;
        crate::types::check::refine(crate::wire::limits::is_plain_path(&self.target), "an absolute path inside the workspace")
            .map_err(|e| e.under("target"))?;
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        crate::types::keys::req_key_with::<String>(obj, "source", &mut out, Self::rule_source);
        crate::types::keys::req_key_with::<String>(obj, "target", &mut out, Self::rule_target);
        Some(out)
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
impl MachineShare {
    fn rule_source(x: &String) -> Result<(), crate::types::check::SchemaError> {
        crate::types::check::text_at_least(x, 1)?;
        crate::types::check::refine(crate::wire::limits::is_plain_path(x), "an absolute path on the computer")?;
        Ok(())
    }
    fn rule_target(x: &String) -> Result<(), crate::types::check::SchemaError> {
        crate::types::check::text_at_least(x, 1)?;
        crate::types::check::refine(crate::wire::limits::is_plain_path(x), "an absolute path inside the workspace")?;
        Ok(())
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
        crate::types::check::refine(crate::wire::limits::is_plain_path(&self.source), "an absolute path on the computer")
            .map_err(|e| e.under("source"))?;
        crate::types::check::text_at_least(&self.target, 1).map_err(|e| e.under("target"))?;
        crate::types::check::refine(crate::wire::limits::is_plain_path(&self.target), "an absolute path inside the workspace")
            .map_err(|e| e.under("target"))?;
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        crate::types::keys::req_key_with::<String>(obj, "source", &mut out, Self::rule_source);
        crate::types::keys::req_key_with::<String>(obj, "target", &mut out, Self::rule_target);
        crate::types::keys::opt_key::<bool>(obj, "readOnly", &mut out);
        Some(out)
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
impl MachineBind {
    fn rule_source(x: &String) -> Result<(), crate::types::check::SchemaError> {
        crate::types::check::text_at_least(x, 1)?;
        crate::types::check::refine(crate::wire::limits::is_plain_path(x), "an absolute path on the computer")?;
        Ok(())
    }
    fn rule_target(x: &String) -> Result<(), crate::types::check::SchemaError> {
        crate::types::check::text_at_least(x, 1)?;
        crate::types::check::refine(crate::wire::limits::is_plain_path(x), "an absolute path inside the workspace")?;
        Ok(())
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

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        crate::types::keys::req_key_with::<String>(obj, "id", &mut out, Self::rule_id);
        Some(out)
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
impl PlaceProvisionRowProject {
    fn rule_id(x: &String) -> Result<(), crate::types::check::SchemaError> {
        crate::types::check::text_at_least(x, 1)?;
        Ok(())
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
    pub ms: Option<JsInt>,
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
            crate::types::check::num_at_least(v.0 .0, 0.0).map_err(|e| e.under("ms"))?;
        }
        self.project.check().map_err(|e| e.under("project"))?;
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        crate::types::keys::req_key_with::<String>(obj, "id", &mut out, Self::rule_id);
        crate::types::keys::req_key_with::<String>(obj, "label", &mut out, Self::rule_label);
        crate::types::keys::req_key::<PlaceProvisionRowOutcome>(obj, "outcome", &mut out);
        crate::types::keys::opt_key::<PlaceProvisionKind>(obj, "kind", &mut out);
        crate::types::keys::opt_key::<PlaceSetupStep>(obj, "step", &mut out);
        crate::types::keys::opt_key::<String>(obj, "note", &mut out);
        crate::types::keys::opt_key::<String>(obj, "fix", &mut out);
        crate::types::keys::opt_key_with::<Vec<String>>(obj, "keys", &mut out, Self::rule_keys);
        crate::types::keys::opt_key_with::<JsInt>(obj, "ms", &mut out, Self::rule_ms);
        crate::types::keys::opt_key::<bool>(obj, "earlier", &mut out);
        crate::types::keys::opt_key::<PlaceProvisionRowProject>(obj, "project", &mut out);
        Some(out)
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
impl PlaceProvisionRow {
    fn rule_id(x: &String) -> Result<(), crate::types::check::SchemaError> {
        crate::types::check::text_at_least(x, 1)?;
        Ok(())
    }
    fn rule_label(x: &String) -> Result<(), crate::types::check::SchemaError> {
        crate::types::check::text_at_least(x, 1)?;
        Ok(())
    }
    fn rule_keys(x: &Option<Vec<String>>) -> Result<(), crate::types::check::SchemaError> {
        if let Some(list) = x {
            for (i, item) in list.iter().enumerate() {
                crate::types::check::text_at_least(item, 1).map_err(|e| e.under(i))?;
            }
        }
        Ok(())
    }
    fn rule_ms(x: &Option<JsInt>) -> Result<(), crate::types::check::SchemaError> {
        if let Some(v) = *x {
            crate::types::check::num_at_least(v.0 .0, 0.0)?;
        }
        Ok(())
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
        crate::types::check::leaf(
            path,
            crate::types::check::Zod::Options(&[
                "floor",
                "agents",
                "signins",
                "clis",
                "skills",
                "mcp",
                "plugins",
                "configs",
                "folders",
                "folderServers",
                "github",
                "context",
            ]),
        )
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
    pub ms: Option<JsInt>,
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
            crate::types::check::num_at_least(v.0 .0, 0.0).map_err(|e| e.under("ms"))?;
        }
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        crate::types::keys::req_key::<PlaceSetupStep>(obj, "step", &mut out);
        crate::types::keys::req_key::<PlaceSetupLineState>(obj, "state", &mut out);
        crate::types::keys::opt_key::<String>(obj, "note", &mut out);
        crate::types::keys::opt_key_with::<JsInt>(obj, "ms", &mut out, Self::rule_ms);
        crate::types::keys::opt_key::<String>(obj, "startedAt", &mut out);
        Some(out)
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
impl PlaceSetupLine {
    fn rule_ms(x: &Option<JsInt>) -> Result<(), crate::types::check::SchemaError> {
        if let Some(v) = *x {
            crate::types::check::num_at_least(v.0 .0, 0.0)?;
        }
        Ok(())
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

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        crate::types::keys::req_key::<String>(obj, "row", &mut out);
        crate::types::keys::req_key::<String>(obj, "label", &mut out);
        crate::types::keys::opt_key::<String>(obj, "url", &mut out);
        crate::types::keys::opt_key::<String>(obj, "code", &mut out);
        crate::types::keys::req_key::<String>(obj, "expiresAt", &mut out);
        crate::types::keys::req_key::<PlaceWaitState>(obj, "state", &mut out);
        Some(out)
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
