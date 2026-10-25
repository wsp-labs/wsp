// SPDX-License-Identifier: AGPL-3.0-only
use super::*;

/// TS: packages/protocol/src/agents-report.ts:335 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServerAdd {
    pub agent: String,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub project: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub command: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub args: Option<Vec<String>>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::types::kept::record_opt",
        serialize_with = "crate::types::check::js_record_opt"
    )]
    pub env: Option<IndexMap<String, String>>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub url: Option<String>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::types::kept::record_opt",
        serialize_with = "crate::types::check::js_record_opt"
    )]
    pub headers: Option<IndexMap<String, String>>,
}
impl Check for ServerAdd {
    fn check(&self) -> Result<(), SchemaError> {
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        crate::types::keys::req_key::<String>(obj, "agent", &mut out);
        crate::types::keys::req_key::<String>(obj, "name", &mut out);
        crate::types::keys::opt_key::<bool>(obj, "project", &mut out);
        crate::types::keys::opt_key::<String>(obj, "command", &mut out);
        crate::types::keys::opt_key::<Vec<String>>(obj, "args", &mut out);
        crate::types::keys::opt_key::<IndexMap<String, String>>(obj, "env", &mut out);
        crate::types::keys::opt_key::<String>(obj, "url", &mut out);
        crate::types::keys::opt_key::<IndexMap<String, String>>(obj, "headers", &mut out);
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "agent" => <String as Check>::describe(rest),
                "name" => <String as Check>::describe(rest),
                "project" => <Option<bool> as Check>::describe(rest),
                "command" => <Option<String> as Check>::describe(rest),
                "args" => <Option<Vec<String>> as Check>::describe(rest),
                "env" => <Option<IndexMap<String, String>> as Check>::describe(rest),
                "url" => <Option<String> as Check>::describe(rest),
                "headers" => <Option<IndexMap<String, String>> as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

/// TS: packages/protocol/src/agents-report.ts:347 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpRow {
    pub agent: String,
    pub name: String,
    pub scope: McpScope,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub file: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub launch: Option<True>,
    pub transport: McpRowTransport,
    pub env_names: Vec<String>,
    pub auth: McpAuth,
    pub enabled: bool,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub in_recipe: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub tools: Option<Vec<McpTool>>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub project: Option<AgentsProject>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub sign_in_line: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub off: Option<String>,
}
impl Check for McpRow {
    /// Nested checks: scope, transport, auth, tools, project.
    fn check(&self) -> Result<(), SchemaError> {
        self.scope.check().map_err(|e| e.under("scope"))?;
        self.transport.check().map_err(|e| e.under("transport"))?;
        self.auth.check().map_err(|e| e.under("auth"))?;
        self.tools.check().map_err(|e| e.under("tools"))?;
        self.project.check().map_err(|e| e.under("project"))?;
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        crate::types::keys::req_key::<String>(obj, "agent", &mut out);
        crate::types::keys::req_key::<String>(obj, "name", &mut out);
        crate::types::keys::req_key::<McpScope>(obj, "scope", &mut out);
        crate::types::keys::opt_key::<String>(obj, "file", &mut out);
        crate::types::keys::opt_key::<True>(obj, "launch", &mut out);
        crate::types::keys::req_key::<McpRowTransport>(obj, "transport", &mut out);
        crate::types::keys::req_key::<Vec<String>>(obj, "envNames", &mut out);
        crate::types::keys::req_key::<McpAuth>(obj, "auth", &mut out);
        crate::types::keys::req_key::<bool>(obj, "enabled", &mut out);
        crate::types::keys::opt_key::<bool>(obj, "inRecipe", &mut out);
        crate::types::keys::opt_key::<Vec<McpTool>>(obj, "tools", &mut out);
        crate::types::keys::opt_key::<AgentsProject>(obj, "project", &mut out);
        crate::types::keys::opt_key::<String>(obj, "signInLine", &mut out);
        crate::types::keys::opt_key::<String>(obj, "off", &mut out);
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "agent" => <String as Check>::describe(rest),
                "name" => <String as Check>::describe(rest),
                "scope" => <McpScope as Check>::describe(rest),
                "file" => <Option<String> as Check>::describe(rest),
                "launch" => None,
                "transport" => <McpRowTransport as Check>::describe(rest),
                "envNames" => <Vec<String> as Check>::describe(rest),
                "auth" => <McpAuth as Check>::describe(rest),
                "enabled" => <bool as Check>::describe(rest),
                "inRecipe" => <Option<bool> as Check>::describe(rest),
                "tools" => <Option<Vec<McpTool>> as Check>::describe(rest),
                "project" => <Option<AgentsProject> as Check>::describe(rest),
                "signInLine" => <Option<String> as Check>::describe(rest),
                "off" => <Option<String> as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

/// TS: packages/protocol/src/agents-report.ts:382 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServerToolsAnswer {
    pub auth: McpAuth,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub tools: Option<Vec<McpTool>>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub holder: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub refused: Option<String>,
    pub read_at: String,
}
impl Check for ServerToolsAnswer {
    /// Nested checks: auth, tools.
    fn check(&self) -> Result<(), SchemaError> {
        self.auth.check().map_err(|e| e.under("auth"))?;
        self.tools.check().map_err(|e| e.under("tools"))?;
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        crate::types::keys::req_key::<McpAuth>(obj, "auth", &mut out);
        crate::types::keys::opt_key::<Vec<McpTool>>(obj, "tools", &mut out);
        crate::types::keys::opt_key::<String>(obj, "holder", &mut out);
        crate::types::keys::opt_key::<String>(obj, "refused", &mut out);
        crate::types::keys::req_key::<String>(obj, "readAt", &mut out);
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "auth" => <McpAuth as Check>::describe(rest),
                "tools" => <Option<Vec<McpTool>> as Check>::describe(rest),
                "holder" => <Option<String> as Check>::describe(rest),
                "refused" => <Option<String> as Check>::describe(rest),
                "readAt" => <String as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

/// TS: packages/protocol/src/agents-report.ts:394 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentsReport {
    pub target: AgentsTarget,
    pub home: String,
    pub user: String,
    pub read_at: String,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub stale: Option<NappingLit>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub pause_mode: Option<PauseMode>,
    pub agents: Vec<AgentRow>,
    pub skills: Vec<SkillRow>,
    pub servers: Vec<McpRow>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub plugins: Option<Vec<PluginRow>>,
    pub refused: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub projects: Option<Vec<AgentsProject>>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub reach: Option<PageReach>,
}
impl Check for AgentsReport {
    /// Nested checks: target, pauseMode, agents, skills, servers, plugins, projects, reach.
    fn check(&self) -> Result<(), SchemaError> {
        self.target.check().map_err(|e| e.under("target"))?;
        self.stale.check().map_err(|e| e.under("stale"))?;
        self.agents.check().map_err(|e| e.under("agents"))?;
        self.skills.check().map_err(|e| e.under("skills"))?;
        self.servers.check().map_err(|e| e.under("servers"))?;
        self.plugins.check().map_err(|e| e.under("plugins"))?;
        self.projects.check().map_err(|e| e.under("projects"))?;
        self.reach.check().map_err(|e| e.under("reach"))?;
        self.pause_mode.check().map_err(|e| e.under("pauseMode"))?;
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        crate::types::keys::req_key::<AgentsTarget>(obj, "target", &mut out);
        crate::types::keys::req_key::<String>(obj, "home", &mut out);
        crate::types::keys::req_key::<String>(obj, "user", &mut out);
        crate::types::keys::req_key::<String>(obj, "readAt", &mut out);
        crate::types::keys::opt_key::<NappingLit>(obj, "stale", &mut out);
        crate::types::keys::opt_key::<PauseMode>(obj, "pauseMode", &mut out);
        crate::types::keys::req_key::<Vec<AgentRow>>(obj, "agents", &mut out);
        crate::types::keys::req_key::<Vec<SkillRow>>(obj, "skills", &mut out);
        crate::types::keys::req_key::<Vec<McpRow>>(obj, "servers", &mut out);
        crate::types::keys::opt_key::<Vec<PluginRow>>(obj, "plugins", &mut out);
        crate::types::keys::req_key::<Vec<String>>(obj, "refused", &mut out);
        crate::types::keys::opt_key::<Vec<AgentsProject>>(obj, "projects", &mut out);
        crate::types::keys::opt_key::<PageReach>(obj, "reach", &mut out);
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "target" => <AgentsTarget as Check>::describe(rest),
                "home" => <String as Check>::describe(rest),
                "user" => <String as Check>::describe(rest),
                "readAt" => <String as Check>::describe(rest),
                "stale" => <Option<NappingLit> as Check>::describe(rest),
                "pauseMode" => None,
                "agents" => <Vec<AgentRow> as Check>::describe(rest),
                "skills" => <Vec<SkillRow> as Check>::describe(rest),
                "servers" => <Vec<McpRow> as Check>::describe(rest),
                "plugins" => <Option<Vec<PluginRow>> as Check>::describe(rest),
                "refused" => <Vec<String> as Check>::describe(rest),
                "projects" => <Option<Vec<AgentsProject>> as Check>::describe(rest),
                "reach" => <Option<PageReach> as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum AgentsSignInEventState {
    #[serde(rename = "running")]
    Running,
    #[serde(rename = "waiting")]
    Waiting,
    #[serde(rename = "signed-in")]
    SignedIn,
    #[serde(rename = "failed")]
    Failed,
}
impl Check for AgentsSignInEventState {
    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        crate::types::check::leaf(path, crate::types::check::Zod::Options(&["running", "waiting", "signed-in", "failed"]))
    }
}

literal!(AgentsSignInEventType, "agents.signIn");

/// `last` of a sign-in run: the event with the `type` key zod writes there in front of it.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AgentsSignInRunLast {
    #[serde(rename = "type")]
    pub kind: AgentsSignInEventType,
    #[serde(flatten)]
    pub event: AgentsSignInEvent,
}
impl Check for AgentsSignInRunLast {
    fn check(&self) -> Result<(), SchemaError> {
        self.event.check()
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        crate::types::keys::req_key::<AgentsSignInEventType>(obj, "type", &mut out);
        out.extend(crate::types::check::issues_of::<AgentsSignInEvent>(value));
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) if key.as_str() == Some("type") => <AgentsSignInEventType as Check>::describe(rest),
            Some(_) => <AgentsSignInEvent as Check>::describe(path),
        }
    }
}

/// TS: packages/protocol/src/agents-report.ts:446 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentsSignInEvent {
    pub sign_in_id: String,
    pub state: AgentsSignInEventState,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub code: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub paste: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub said: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub pty_id: Option<String>,
}
impl AgentsSignInEvent {
    /// The `type` key's value: the enum holding this struct writes and reads it as its tag.
    pub const TYPE: &'static str = "agents.signIn";
}
impl Check for AgentsSignInEvent {
    fn check(&self) -> Result<(), SchemaError> {
        self.state.check().map_err(|e| e.under("state"))?;
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        crate::types::keys::req_key::<String>(obj, "signInId", &mut out);
        crate::types::keys::req_key::<AgentsSignInEventState>(obj, "state", &mut out);
        crate::types::keys::opt_key::<String>(obj, "url", &mut out);
        crate::types::keys::opt_key::<String>(obj, "code", &mut out);
        crate::types::keys::opt_key::<bool>(obj, "paste", &mut out);
        crate::types::keys::opt_key::<String>(obj, "said", &mut out);
        crate::types::keys::opt_key::<String>(obj, "ptyId", &mut out);
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "signInId" => <String as Check>::describe(rest),
                "state" => <AgentsSignInEventState as Check>::describe(rest),
                "url" => <Option<String> as Check>::describe(rest),
                "code" => <Option<String> as Check>::describe(rest),
                "paste" => <Option<bool> as Check>::describe(rest),
                "said" => <Option<String> as Check>::describe(rest),
                "ptyId" => <Option<String> as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

/// TS: packages/protocol/src/agents-report.ts:461 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentsSignInRun {
    pub sign_in_id: String,
    pub target: AgentsTarget,
    pub agent: String,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub server: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub scope: Option<McpScope>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub project: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub last: Option<AgentsSignInRunLast>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub ended: Option<True>,
}
impl Check for AgentsSignInRun {
    /// Nested checks: target, scope, last.
    fn check(&self) -> Result<(), SchemaError> {
        self.target.check().map_err(|e| e.under("target"))?;
        self.scope.check().map_err(|e| e.under("scope"))?;
        self.last.check().map_err(|e| e.under("last"))?;
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        crate::types::keys::req_key::<String>(obj, "signInId", &mut out);
        crate::types::keys::req_key::<AgentsTarget>(obj, "target", &mut out);
        crate::types::keys::req_key::<String>(obj, "agent", &mut out);
        crate::types::keys::opt_key::<String>(obj, "server", &mut out);
        crate::types::keys::opt_key::<McpScope>(obj, "scope", &mut out);
        crate::types::keys::opt_key::<String>(obj, "project", &mut out);
        crate::types::keys::opt_key::<AgentsSignInRunLast>(obj, "last", &mut out);
        crate::types::keys::opt_key::<True>(obj, "ended", &mut out);
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "signInId" => <String as Check>::describe(rest),
                "target" => <AgentsTarget as Check>::describe(rest),
                "agent" => <String as Check>::describe(rest),
                "server" => <Option<String> as Check>::describe(rest),
                "scope" => <Option<McpScope> as Check>::describe(rest),
                "project" => <Option<String> as Check>::describe(rest),
                "last" => <Option<AgentsSignInRunLast> as Check>::describe(rest),
                "ended" => None,
                _ => None,
            },
        }
    }
}

/// TS: packages/protocol/src/agents-report.ts:478 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentsChangedEvent {
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub target: Option<AgentsTarget>,
}
impl AgentsChangedEvent {
    /// The `type` key's value: the enum holding this struct writes and reads it as its tag.
    pub const TYPE: &'static str = "agents.changed";
}
impl Check for AgentsChangedEvent {
    /// Nested checks: target.
    fn check(&self) -> Result<(), SchemaError> {
        self.target.check().map_err(|e| e.under("target"))?;
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        crate::types::keys::opt_key::<AgentsTarget>(obj, "target", &mut out);
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "target" => <Option<AgentsTarget> as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

/// TS: packages/protocol/src/agents-report.ts:484 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SignInLine {
    pub command: String,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::types::kept::record_opt",
        serialize_with = "crate::types::check::js_record_opt"
    )]
    pub env: Option<IndexMap<String, String>>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub prepare: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub status: Option<String>,
}
impl Check for SignInLine {
    fn check(&self) -> Result<(), SchemaError> {
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        crate::types::keys::req_key::<String>(obj, "command", &mut out);
        crate::types::keys::opt_key::<IndexMap<String, String>>(obj, "env", &mut out);
        crate::types::keys::opt_key::<String>(obj, "prepare", &mut out);
        crate::types::keys::opt_key::<String>(obj, "status", &mut out);
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "command" => <String as Check>::describe(rest),
                "env" => <Option<IndexMap<String, String>> as Check>::describe(rest),
                "prepare" => <Option<String> as Check>::describe(rest),
                "status" => <Option<String> as Check>::describe(rest),
                _ => None,
            },
        }
    }
}
