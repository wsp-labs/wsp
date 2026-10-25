// SPDX-License-Identifier: AGPL-3.0-only
use super::*;

literal!(UnknownLit, "unknown");

literal!(NappingLit, "napping");

/// TS: packages/protocol/src/agents-report.ts:120 at a3b870a6e.
pub const SKILL_PREVIEW_BYTES: u64 = 65_536;

/// TS: packages/protocol/src/agents-report.ts:131 at a3b870a6e.
pub const SKILLS_SEARCH_EMPTY_REFUSAL: &str = "Type something to search skills.sh for.";

/// TS: packages/protocol/src/agents-report.ts:176 at a3b870a6e.
pub const UNCLOSED_QUOTE_REFUSAL: &str = "The command has a quote it never closes, so nothing was written.";

/// TS: packages/protocol/src/agents-report.ts:182 at a3b870a6e.
pub const SERVER_NAME_REFUSAL: &str = "A server's name needs at least one character and no control character, so nothing was written.";

/// TS: packages/protocol/src/agents-report.ts:195 at a3b870a6e.
pub const CONFIG_READ_MAX_BYTES: u64 = 33_554_432;

/// TS: packages/protocol/src/agents-report.ts:197 at a3b870a6e.
pub const CONFIG_NOT_PLAIN: &str = "it is not a plain file, or it is a link out of the folder it belongs to, and wsp reads neither";

/// TS: packages/protocol/src/agents-report.ts:199 at a3b870a6e.
pub const CONFIG_PAST_CAP: &str = "it is over 32 MB, more than wsp reads of an agent's config";

/// TS: packages/protocol/src/agents-report.ts:474 at a3b870a6e.
pub const SIGN_IN_ENDED_KEPT_MS: u64 = 600_000;

/// TS: packages/protocol/src/agents-report.ts:514 at a3b870a6e.
pub const ADD_TOOLS_HERE_REFUSAL: &str =
    "The wsp tools go into an agent's config on this computer; a thread on any other computer is handed them with every turn.";

/// TS: packages/protocol/src/agents-report.ts:530 at a3b870a6e.
pub const CONTROL_SIGN_IN_REFUSAL: &str = "That name holds a control character, so no sign-in runs for it.";

/// TS: packages/protocol/src/agents-report.ts:533 at a3b870a6e.
pub const NO_SIGN_IN_REFUSAL: &str = "That sign-in is not running any more.";

/// TS: packages/protocol/src/agents-report.ts:555 at a3b870a6e.
pub const SIGN_IN_LINE_REFUSAL: &str =
    "only the host's own command line asks for a sign-in's line; run wsp agents signin on the computer the host runs on";

/// TS: packages/protocol/src/agents-report.ts:558 at a3b870a6e.
pub const AGENTS_KEY_REFUSAL: &str = "only a socket holding this host's own token may put a key in its vault; use the app on the computer the host runs on, or wsp agents key there";

/// TS: packages/protocol/src/agents-report.ts:21 at a3b870a6e.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum AgentSignInState {
    #[serde(rename = "signed-in")]
    SignedIn,
    #[serde(rename = "vault-key")]
    VaultKey,
    #[serde(rename = "none")]
    None,
}
impl Check for AgentSignInState {
    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        crate::types::check::leaf(path, crate::types::check::Zod::Options(&["signed-in", "vault-key", "none"]))
    }
}

/// TS: packages/protocol/src/agents-report.ts:26 at a3b870a6e.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum AgentSignInKind {
    #[serde(rename = "api-key")]
    ApiKey,
    #[serde(rename = "subscription")]
    Subscription,
    #[serde(rename = "oauth")]
    Oauth,
}
impl Check for AgentSignInKind {
    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        crate::types::check::leaf(path, crate::types::check::Zod::Options(&["api-key", "subscription", "oauth"]))
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AgentsTargetV0 {
    pub place_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub project: Option<String>,
}
impl Check for AgentsTargetV0 {
    /// Strict: an unknown key is refused (zod .strict()).
    fn check(&self) -> Result<(), SchemaError> {
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        crate::types::keys::req_key::<String>(obj, "placeId", &mut out);
        crate::types::keys::opt_key::<String>(obj, "project", &mut out);
        let known: &[&str] = &["placeId", "project"];
        let extra: Vec<String> = obj.keys().filter(|k| !known.contains(&k.as_str())).cloned().collect();
        if !extra.is_empty() {
            out.push(crate::types::check::unrecognized(Vec::new(), extra));
        }
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "placeId" => <String as Check>::describe(rest),
                "project" => <Option<String> as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AgentsTargetV1 {
    pub workspace_id: String,
}
impl Check for AgentsTargetV1 {
    /// Strict: an unknown key is refused (zod .strict()).
    fn check(&self) -> Result<(), SchemaError> {
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        crate::types::keys::req_key::<String>(obj, "workspaceId", &mut out);
        let known: &[&str] = &["workspaceId"];
        let extra: Vec<String> = obj.keys().filter(|k| !known.contains(&k.as_str())).cloned().collect();
        if !extra.is_empty() {
            out.push(crate::types::check::unrecognized(Vec::new(), extra));
        }
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "workspaceId" => <String as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

/// TS: packages/protocol/src/agents-report.ts:32 at a3b870a6e.
/// A zod union: the first variant that parses wins, in this order.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(untagged)]
pub enum AgentsTarget {
    V0(AgentsTargetV0),
    V1(AgentsTargetV1),
}

crate::zod_union!(AgentsTarget { V0(AgentsTargetV0), V1(AgentsTargetV1) }

);

impl Check for AgentsTarget {
    fn check(&self) -> Result<(), SchemaError> {
        match self {
            AgentsTarget::V0(v) => v.check(),
            AgentsTarget::V1(v) => v.check(),
        }
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        Some(crate::types::check::union_answer(vec![
            crate::types::check::issues_of::<AgentsTargetV0>(value),
            crate::types::check::issues_of::<AgentsTargetV1>(value),
        ]))
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        if path.is_empty() {
            return Some(crate::types::check::Zod::Union);
        }
        <AgentsTargetV0 as Check>::describe(path).or_else(|| <AgentsTargetV1 as Check>::describe(path))
    }
}

/// TS: packages/protocol/src/agents-report.ts:41 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentsProject {
    pub id: String,
    pub name: String,
    pub path: String,
}
impl Check for AgentsProject {
    fn check(&self) -> Result<(), SchemaError> {
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        crate::types::keys::req_key::<String>(obj, "id", &mut out);
        crate::types::keys::req_key::<String>(obj, "name", &mut out);
        crate::types::keys::req_key::<String>(obj, "path", &mut out);
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "id" => <String as Check>::describe(rest),
                "name" => <String as Check>::describe(rest),
                "path" => <String as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

/// TS: packages/protocol/src/agents-report.ts:53 at a3b870a6e.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum AgentRoad {
    #[serde(rename = "wsp")]
    Wsp,
    #[serde(rename = "own")]
    Own,
    #[serde(rename = "shim")]
    Shim,
    #[serde(rename = "none")]
    None,
}
impl Check for AgentRoad {
    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        crate::types::check::leaf(path, crate::types::check::Zod::Options(&["wsp", "own", "shim", "none"]))
    }
}

/// TS: packages/protocol/src/agents-report.ts:58 at a3b870a6e.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SignInRoad {
    #[serde(rename = "device")]
    Device,
    #[serde(rename = "code")]
    Code,
    #[serde(rename = "key")]
    Key,
    #[serde(rename = "token")]
    Token,
    #[serde(rename = "terminal")]
    Terminal,
    #[serde(rename = "none")]
    None,
}
impl Check for SignInRoad {
    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        crate::types::check::leaf(path, crate::types::check::Zod::Options(&["device", "code", "key", "token", "terminal", "none"]))
    }
}

/// A zod union: the first variant that parses wins, in this order.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(untagged)]
pub enum AgentRowSignIn {
    AgentSignInState(AgentSignInState),
    V1(UnknownLit),
}

crate::zod_union!(AgentRowSignIn { AgentSignInState(AgentSignInState), V1(UnknownLit) }

);

impl Check for AgentRowSignIn {
    fn check(&self) -> Result<(), SchemaError> {
        match self {
            AgentRowSignIn::AgentSignInState(v) => v.check(),
            AgentRowSignIn::V1(v) => v.check(),
        }
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        Some(crate::types::check::union_answer(vec![
            crate::types::check::issues_of::<AgentSignInState>(value),
            crate::types::check::issues_of::<UnknownLit>(value),
        ]))
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        if path.is_empty() {
            return Some(crate::types::check::Zod::Union);
        }
        <AgentSignInState as Check>::describe(path).or_else(|| <UnknownLit as Check>::describe(path))
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentRowUpdate {
    pub to: String,
    pub command: String,
}
impl Check for AgentRowUpdate {
    fn check(&self) -> Result<(), SchemaError> {
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        crate::types::keys::req_key::<String>(obj, "to", &mut out);
        crate::types::keys::req_key::<String>(obj, "command", &mut out);
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "to" => <String as Check>::describe(rest),
                "command" => <String as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

/// TS: packages/protocol/src/agents-report.ts:61 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentRow {
    pub id: String,
    pub name: String,
    pub installed: bool,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub version: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub latest: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub pinned: Option<String>,
    pub road: AgentRoad,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub installs_on_first_run: Option<True>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub version_unread: Option<True>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub via: Option<String>,
    pub sign_in: AgentRowSignIn,
    pub sign_in_road: SignInRoad,
    pub wsp_tools: bool,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub sign_in_detail: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub sign_in_kind: Option<AgentSignInKind>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub sign_in_plan: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub update: Option<AgentRowUpdate>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub setup: Option<AgentSetupView>,
}
impl Check for AgentRow {
    /// Nested checks: road, signIn, signInRoad, signInKind, update, setup.
    fn check(&self) -> Result<(), SchemaError> {
        self.road.check().map_err(|e| e.under("road"))?;
        self.sign_in.check().map_err(|e| e.under("signIn"))?;
        self.sign_in_road.check().map_err(|e| e.under("signInRoad"))?;
        self.sign_in_kind.check().map_err(|e| e.under("signInKind"))?;
        self.update.check().map_err(|e| e.under("update"))?;
        self.setup.check().map_err(|e| e.under("setup"))?;
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        crate::types::keys::req_key::<String>(obj, "id", &mut out);
        crate::types::keys::req_key::<String>(obj, "name", &mut out);
        crate::types::keys::req_key::<bool>(obj, "installed", &mut out);
        crate::types::keys::opt_key::<String>(obj, "version", &mut out);
        crate::types::keys::opt_key::<String>(obj, "latest", &mut out);
        crate::types::keys::opt_key::<String>(obj, "pinned", &mut out);
        crate::types::keys::req_key::<AgentRoad>(obj, "road", &mut out);
        crate::types::keys::opt_key::<True>(obj, "installsOnFirstRun", &mut out);
        crate::types::keys::opt_key::<True>(obj, "versionUnread", &mut out);
        crate::types::keys::opt_key::<String>(obj, "path", &mut out);
        crate::types::keys::opt_key::<String>(obj, "via", &mut out);
        crate::types::keys::req_key::<AgentRowSignIn>(obj, "signIn", &mut out);
        crate::types::keys::req_key::<SignInRoad>(obj, "signInRoad", &mut out);
        crate::types::keys::req_key::<bool>(obj, "wspTools", &mut out);
        crate::types::keys::opt_key::<String>(obj, "signInDetail", &mut out);
        crate::types::keys::opt_key::<AgentSignInKind>(obj, "signInKind", &mut out);
        crate::types::keys::opt_key::<String>(obj, "signInPlan", &mut out);
        crate::types::keys::opt_key::<AgentRowUpdate>(obj, "update", &mut out);
        crate::types::keys::opt_key::<AgentSetupView>(obj, "setup", &mut out);
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "id" => <String as Check>::describe(rest),
                "name" => <String as Check>::describe(rest),
                "installed" => <bool as Check>::describe(rest),
                "version" => <Option<String> as Check>::describe(rest),
                "latest" => <Option<String> as Check>::describe(rest),
                "pinned" => <Option<String> as Check>::describe(rest),
                "road" => <AgentRoad as Check>::describe(rest),
                "installsOnFirstRun" => None,
                "versionUnread" => None,
                "path" => <Option<String> as Check>::describe(rest),
                "via" => <Option<String> as Check>::describe(rest),
                "signIn" => <AgentRowSignIn as Check>::describe(rest),
                "signInRoad" => <SignInRoad as Check>::describe(rest),
                "wspTools" => <bool as Check>::describe(rest),
                "signInDetail" => <Option<String> as Check>::describe(rest),
                "signInKind" => <Option<AgentSignInKind> as Check>::describe(rest),
                "signInPlan" => <Option<String> as Check>::describe(rest),
                "update" => <Option<AgentRowUpdate> as Check>::describe(rest),
                "setup" => <Option<AgentSetupView> as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

/// TS: packages/protocol/src/agents-report.ts:103 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillPath {
    pub path: String,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub agent: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub link_to: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub off: Option<True>,
}
impl Check for SkillPath {
    fn check(&self) -> Result<(), SchemaError> {
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        crate::types::keys::req_key::<String>(obj, "path", &mut out);
        crate::types::keys::opt_key::<String>(obj, "agent", &mut out);
        crate::types::keys::opt_key::<String>(obj, "linkTo", &mut out);
        crate::types::keys::opt_key::<True>(obj, "off", &mut out);
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "path" => <String as Check>::describe(rest),
                "agent" => <Option<String> as Check>::describe(rest),
                "linkTo" => <Option<String> as Check>::describe(rest),
                "off" => None,
                _ => None,
            },
        }
    }
}

/// TS: packages/protocol/src/agents-report.ts:106 at a3b870a6e.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SkillScope {
    #[serde(rename = "user")]
    User,
    #[serde(rename = "project")]
    Project,
    #[serde(rename = "plugin")]
    Plugin,
}
impl Check for SkillScope {
    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        crate::types::check::leaf(path, crate::types::check::Zod::Options(&["user", "project", "plugin"]))
    }
}

/// TS: packages/protocol/src/agents-report.ts:112 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillRow {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub description: Option<String>,
    pub paths: Vec<SkillPath>,
    pub scope: SkillScope,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub project: Option<AgentsProject>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub plugin: Option<String>,
}
impl Check for SkillRow {
    /// Rules: paths: at least 1 items.
    /// Nested checks: paths, scope, project.
    fn check(&self) -> Result<(), SchemaError> {
        crate::types::check::items_at_least(self.paths.len(), 1).map_err(|e| e.under("paths"))?;
        self.paths.check().map_err(|e| e.under("paths"))?;
        self.scope.check().map_err(|e| e.under("scope"))?;
        self.project.check().map_err(|e| e.under("project"))?;
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        crate::types::keys::req_key::<String>(obj, "name", &mut out);
        crate::types::keys::opt_key::<String>(obj, "description", &mut out);
        crate::types::keys::req_key_with::<Vec<SkillPath>>(obj, "paths", &mut out, Self::rule_paths);
        crate::types::keys::req_key::<SkillScope>(obj, "scope", &mut out);
        crate::types::keys::opt_key::<AgentsProject>(obj, "project", &mut out);
        crate::types::keys::opt_key::<String>(obj, "plugin", &mut out);
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "name" => <String as Check>::describe(rest),
                "description" => <Option<String> as Check>::describe(rest),
                "paths" => <Vec<SkillPath> as Check>::describe(rest),
                "scope" => <SkillScope as Check>::describe(rest),
                "project" => <Option<AgentsProject> as Check>::describe(rest),
                "plugin" => <Option<String> as Check>::describe(rest),
                _ => None,
            },
        }
    }
}
impl SkillRow {
    fn rule_paths(x: &Vec<SkillPath>) -> Result<(), crate::types::check::SchemaError> {
        crate::types::check::items_at_least((*x).len(), 1)?;
        Ok(())
    }
}

/// TS: packages/protocol/src/agents-report.ts:116 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillHit {
    pub id: String,
    pub source: String,
    pub skill_id: String,
    pub name: String,
    pub installs: JsInt,
}
impl Check for SkillHit {
    /// Rules: installs: >= 0.
    fn check(&self) -> Result<(), SchemaError> {
        crate::types::check::num_at_least(self.installs.0 .0, 0.0).map_err(|e| e.under("installs"))?;
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        crate::types::keys::req_key::<String>(obj, "id", &mut out);
        crate::types::keys::req_key::<String>(obj, "source", &mut out);
        crate::types::keys::req_key::<String>(obj, "skillId", &mut out);
        crate::types::keys::req_key::<String>(obj, "name", &mut out);
        crate::types::keys::req_key_with::<JsInt>(obj, "installs", &mut out, Self::rule_installs);
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "id" => <String as Check>::describe(rest),
                "source" => <String as Check>::describe(rest),
                "skillId" => <String as Check>::describe(rest),
                "name" => <String as Check>::describe(rest),
                "installs" => <i64 as Check>::describe(rest),
                _ => None,
            },
        }
    }
}
impl SkillHit {
    fn rule_installs(x: &JsInt) -> Result<(), crate::types::check::SchemaError> {
        crate::types::check::num_at_least(x.0 .0, 0.0)?;
        Ok(())
    }
}

/// TS: packages/protocol/src/agents-report.ts:123 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillPreview {
    pub text: String,
    pub size: JsInt,
}
impl Check for SkillPreview {
    /// Rules: size: >= 0.
    fn check(&self) -> Result<(), SchemaError> {
        crate::types::check::num_at_least(self.size.0 .0, 0.0).map_err(|e| e.under("size"))?;
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        crate::types::keys::req_key::<String>(obj, "text", &mut out);
        crate::types::keys::req_key_with::<JsInt>(obj, "size", &mut out, Self::rule_size);
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "text" => <String as Check>::describe(rest),
                "size" => <i64 as Check>::describe(rest),
                _ => None,
            },
        }
    }
}
impl SkillPreview {
    fn rule_size(x: &JsInt) -> Result<(), crate::types::check::SchemaError> {
        crate::types::check::num_at_least(x.0 .0, 0.0)?;
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillAddedAgentsItem {
    pub agent: String,
    pub path: String,
}
impl Check for SkillAddedAgentsItem {
    fn check(&self) -> Result<(), SchemaError> {
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        crate::types::keys::req_key::<String>(obj, "agent", &mut out);
        crate::types::keys::req_key::<String>(obj, "path", &mut out);
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "agent" => <String as Check>::describe(rest),
                "path" => <String as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

/// TS: packages/protocol/src/agents-report.ts:127 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillAdded {
    pub path: String,
    pub agents: Vec<SkillAddedAgentsItem>,
}
impl Check for SkillAdded {
    /// Nested checks: agents.
    fn check(&self) -> Result<(), SchemaError> {
        self.agents.check().map_err(|e| e.under("agents"))?;
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        crate::types::keys::req_key::<String>(obj, "path", &mut out);
        crate::types::keys::req_key::<Vec<SkillAddedAgentsItem>>(obj, "agents", &mut out);
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "path" => <String as Check>::describe(rest),
                "agents" => <Vec<SkillAddedAgentsItem> as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

/// TS: packages/protocol/src/agents-report.ts:224 at a3b870a6e.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PluginScope {
    #[serde(rename = "user")]
    User,
    #[serde(rename = "project")]
    Project,
    #[serde(rename = "local")]
    Local,
}
impl Check for PluginScope {
    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        crate::types::check::leaf(path, crate::types::check::Zod::Options(&["user", "project", "local"]))
    }
}

/// TS: packages/protocol/src/agents-report.ts:229 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginBrings {
    pub skills: Vec<String>,
    pub commands: Vec<String>,
    pub subagents: Vec<String>,
    pub hooks: Vec<String>,
    pub servers: Vec<String>,
    pub lsp: Vec<String>,
    pub apps: Vec<String>,
}
impl Check for PluginBrings {
    fn check(&self) -> Result<(), SchemaError> {
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        crate::types::keys::req_key::<Vec<String>>(obj, "skills", &mut out);
        crate::types::keys::req_key::<Vec<String>>(obj, "commands", &mut out);
        crate::types::keys::req_key::<Vec<String>>(obj, "subagents", &mut out);
        crate::types::keys::req_key::<Vec<String>>(obj, "hooks", &mut out);
        crate::types::keys::req_key::<Vec<String>>(obj, "servers", &mut out);
        crate::types::keys::req_key::<Vec<String>>(obj, "lsp", &mut out);
        crate::types::keys::req_key::<Vec<String>>(obj, "apps", &mut out);
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "skills" => <Vec<String> as Check>::describe(rest),
                "commands" => <Vec<String> as Check>::describe(rest),
                "subagents" => <Vec<String> as Check>::describe(rest),
                "hooks" => <Vec<String> as Check>::describe(rest),
                "servers" => <Vec<String> as Check>::describe(rest),
                "lsp" => <Vec<String> as Check>::describe(rest),
                "apps" => <Vec<String> as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

/// TS: packages/protocol/src/agents-report.ts:243 at a3b870a6e.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PluginMissing {
    #[serde(rename = "folder")]
    Folder,
    #[serde(rename = "marketplace")]
    Marketplace,
    #[serde(rename = "unlisted")]
    Unlisted,
    #[serde(rename = "uninstalled")]
    Uninstalled,
}
impl Check for PluginMissing {
    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        crate::types::check::leaf(path, crate::types::check::Zod::Options(&["folder", "marketplace", "unlisted", "uninstalled"]))
    }
}

/// TS: packages/protocol/src/agents-report.ts:249 at a3b870a6e. The fields sit in the order the host writes a row
/// (`pluginRow`, collect/src/detect/plugins.ts:93): `project`, `path` and `setIn` after `brings`, not in the schema's order.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginRow {
    pub agent: String,
    pub id: String,
    pub name: String,
    pub marketplace: String,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub version: Option<String>,
    pub scope: PluginScope,
    pub on: bool,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub missing: Option<PluginMissing>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub source: Option<String>,
    pub brings: PluginBrings,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub project: Option<AgentsProject>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub set_in: Option<String>,
}
impl Check for PluginRow {
    /// Nested checks: scope, project, missing, brings.
    fn check(&self) -> Result<(), SchemaError> {
        self.scope.check().map_err(|e| e.under("scope"))?;
        self.project.check().map_err(|e| e.under("project"))?;
        self.missing.check().map_err(|e| e.under("missing"))?;
        self.brings.check().map_err(|e| e.under("brings"))?;
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        crate::types::keys::req_key::<String>(obj, "agent", &mut out);
        crate::types::keys::req_key::<String>(obj, "id", &mut out);
        crate::types::keys::req_key::<String>(obj, "name", &mut out);
        crate::types::keys::req_key::<String>(obj, "marketplace", &mut out);
        crate::types::keys::opt_key::<String>(obj, "version", &mut out);
        crate::types::keys::req_key::<PluginScope>(obj, "scope", &mut out);
        crate::types::keys::opt_key::<AgentsProject>(obj, "project", &mut out);
        crate::types::keys::req_key::<bool>(obj, "on", &mut out);
        crate::types::keys::opt_key::<PluginMissing>(obj, "missing", &mut out);
        crate::types::keys::opt_key::<String>(obj, "path", &mut out);
        crate::types::keys::opt_key::<String>(obj, "setIn", &mut out);
        crate::types::keys::opt_key::<String>(obj, "description", &mut out);
        crate::types::keys::opt_key::<String>(obj, "source", &mut out);
        crate::types::keys::req_key::<PluginBrings>(obj, "brings", &mut out);
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "agent" => <String as Check>::describe(rest),
                "id" => <String as Check>::describe(rest),
                "name" => <String as Check>::describe(rest),
                "marketplace" => <String as Check>::describe(rest),
                "version" => <Option<String> as Check>::describe(rest),
                "scope" => <PluginScope as Check>::describe(rest),
                "project" => <Option<AgentsProject> as Check>::describe(rest),
                "on" => <bool as Check>::describe(rest),
                "missing" => <Option<PluginMissing> as Check>::describe(rest),
                "path" => <Option<String> as Check>::describe(rest),
                "setIn" => <Option<String> as Check>::describe(rest),
                "description" => <Option<String> as Check>::describe(rest),
                "source" => <Option<String> as Check>::describe(rest),
                "brings" => <PluginBrings as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

/// TS: packages/protocol/src/agents-report.ts:268 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginAsk {
    pub agent: String,
    pub plugin: String,
}
impl Check for PluginAsk {
    fn check(&self) -> Result<(), SchemaError> {
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        crate::types::keys::req_key::<String>(obj, "agent", &mut out);
        crate::types::keys::req_key::<String>(obj, "plugin", &mut out);
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "agent" => <String as Check>::describe(rest),
                "plugin" => <String as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

/// TS: packages/protocol/src/agents-report.ts:299 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpToolParam {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub r#type: Option<String>,
    pub required: bool,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub description: Option<String>,
}
impl Check for McpToolParam {
    fn check(&self) -> Result<(), SchemaError> {
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        crate::types::keys::req_key::<String>(obj, "name", &mut out);
        crate::types::keys::opt_key::<String>(obj, "type", &mut out);
        crate::types::keys::req_key::<bool>(obj, "required", &mut out);
        crate::types::keys::opt_key::<String>(obj, "description", &mut out);
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "name" => <String as Check>::describe(rest),
                "type" => <Option<String> as Check>::describe(rest),
                "required" => <bool as Check>::describe(rest),
                "description" => <Option<String> as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

/// TS: packages/protocol/src/agents-report.ts:303 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpTool {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub params: Option<Vec<McpToolParam>>,
}
impl Check for McpTool {
    /// Nested checks: params.
    fn check(&self) -> Result<(), SchemaError> {
        self.params.check().map_err(|e| e.under("params"))?;
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        crate::types::keys::req_key::<String>(obj, "name", &mut out);
        crate::types::keys::opt_key::<String>(obj, "description", &mut out);
        crate::types::keys::opt_key::<Vec<McpToolParam>>(obj, "params", &mut out);
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "name" => <String as Check>::describe(rest),
                "description" => <Option<String> as Check>::describe(rest),
                "params" => <Option<Vec<McpToolParam>> as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpRowTransportStdio {
    pub line: String,
}
impl McpRowTransportStdio {
    /// The `kind` key's value: the enum holding this struct writes and reads it as its tag.
    pub const KIND: &'static str = "stdio";
}
impl Check for McpRowTransportStdio {
    fn check(&self) -> Result<(), SchemaError> {
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        crate::types::keys::req_key::<String>(obj, "line", &mut out);
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "line" => <String as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpRowTransportHttp {
    pub host: String,
}
impl McpRowTransportHttp {
    /// The `kind` key's value: the enum holding this struct writes and reads it as its tag.
    pub const KIND: &'static str = "http";
}
impl Check for McpRowTransportHttp {
    fn check(&self) -> Result<(), SchemaError> {
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        crate::types::keys::req_key::<String>(obj, "host", &mut out);
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "host" => <String as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

/// TS: packages/protocol/src/agents-report.ts:308 at a3b870a6e.
/// zod's discriminatedUnion on `kind`: each variant's struct leaves that key to this tag.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind")]
pub enum McpRowTransport {
    #[serde(rename = "stdio")]
    Stdio(McpRowTransportStdio),
    #[serde(rename = "http")]
    Http(McpRowTransportHttp),
}
impl Check for McpRowTransport {
    fn check(&self) -> Result<(), SchemaError> {
        match self {
            McpRowTransport::Stdio(v) => v.check(),
            McpRowTransport::Http(v) => v.check(),
        }
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let Some(tag) = obj.get("kind") else {
            return Some(vec![crate::types::check::missing::<Self>("kind")]);
        };
        match tag.as_str() {
            Some("stdio") => {
                let mut body = obj.clone();
                body.shift_remove("kind");
                Some(crate::types::check::issues_of::<McpRowTransportStdio>(&serde_json::Value::Object(body)))
            }
            Some("http") => {
                let mut body = obj.clone();
                body.shift_remove("kind");
                Some(crate::types::check::issues_of::<McpRowTransportHttp>(&serde_json::Value::Object(body)))
            }
            _ => Some(vec![crate::types::check::unknown_tag::<Self>("kind", tag)]),
        }
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        if path.is_empty() {
            return Some(crate::types::check::Zod::Word("object"));
        }
        if path.len() == 1 && path[0].as_str() == Some("kind") {
            return Some(crate::types::check::Zod::Tags("kind", &["stdio", "http"]));
        }
        <McpRowTransportStdio as Check>::describe(path).or_else(|| <McpRowTransportHttp as Check>::describe(path))
    }
}

/// TS: packages/protocol/src/agents-report.ts:314 at a3b870a6e.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum McpAuth {
    #[serde(rename = "open")]
    Open,
    #[serde(rename = "env-key")]
    EnvKey,
    #[serde(rename = "connected")]
    Connected,
    #[serde(rename = "signed-in")]
    SignedIn,
    #[serde(rename = "needs-sign-in")]
    NeedsSignIn,
    #[serde(rename = "failed")]
    Failed,
    #[serde(rename = "unknown")]
    Unknown,
}
impl Check for McpAuth {
    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        crate::types::check::leaf(
            path,
            crate::types::check::Zod::Options(&["open", "env-key", "connected", "signed-in", "needs-sign-in", "failed", "unknown"]),
        )
    }
}

/// TS: packages/protocol/src/agents-report.ts:319 at a3b870a6e.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PageReach {
    #[serde(rename = "here")]
    Here,
    #[serde(rename = "relay")]
    Relay,
    #[serde(rename = "none")]
    None,
}
impl Check for PageReach {
    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        crate::types::check::leaf(path, crate::types::check::Zod::Options(&["here", "relay", "none"]))
    }
}

/// TS: packages/protocol/src/agents-report.ts:324 at a3b870a6e.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum McpScope {
    #[serde(rename = "user")]
    User,
    #[serde(rename = "home")]
    Home,
    #[serde(rename = "local")]
    Local,
    #[serde(rename = "project")]
    Project,
}
impl Check for McpScope {
    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        crate::types::check::leaf(path, crate::types::check::Zod::Options(&["user", "home", "local", "project"]))
    }
}

/// TS: packages/protocol/src/agents-report.ts:329 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServerAsk {
    pub agent: String,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub scope: Option<McpScope>,
}
impl Check for ServerAsk {
    /// Nested checks: scope.
    fn check(&self) -> Result<(), SchemaError> {
        self.scope.check().map_err(|e| e.under("scope"))?;
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        crate::types::keys::req_key::<String>(obj, "agent", &mut out);
        crate::types::keys::req_key::<String>(obj, "name", &mut out);
        crate::types::keys::opt_key::<McpScope>(obj, "scope", &mut out);
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "agent" => <String as Check>::describe(rest),
                "name" => <String as Check>::describe(rest),
                "scope" => <Option<McpScope> as Check>::describe(rest),
                _ => None,
            },
        }
    }
}
