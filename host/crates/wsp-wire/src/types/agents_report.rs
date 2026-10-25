// SPDX-License-Identifier: AGPL-3.0-only
//! Port of packages/protocol/src/agents-report.ts at a3b870a6e: its zod shapes and plain constants, written by the generator.

use serde::{Deserialize, Serialize};

use crate::literal;
use crate::types::check::{Check, SchemaError};
use crate::types::js_number::JsInt;
use crate::types::thread_defaults::AgentSetupView;
use indexmap::IndexMap;
use crate::wire::capabilities::PauseMode;
use wsp_frames::True;

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
        match obj.get("placeId") { None => out.push(crate::types::check::missing::<Self>("placeId")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("placeId"))), } } }
        if let Some(v) = obj.get("project") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("project"))), } }
        let known: &[&str] = &["placeId", "project"];
        let extra: Vec<String> = obj.keys().filter(|k| !known.contains(&k.as_str())).cloned().collect();
        if !extra.is_empty() { out.push(crate::types::check::unrecognized(Vec::new(), extra)); }
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
        match obj.get("workspaceId") { None => out.push(crate::types::check::missing::<Self>("workspaceId")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("workspaceId"))), } } }
        let known: &[&str] = &["workspaceId"];
        let extra: Vec<String> = obj.keys().filter(|k| !known.contains(&k.as_str())).cloned().collect();
        if !extra.is_empty() { out.push(crate::types::check::unrecognized(Vec::new(), extra)); }
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
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum AgentsTarget {
    V0(AgentsTargetV0),
    V1(AgentsTargetV1),
}

impl Check for AgentsTarget {
    fn check(&self) -> Result<(), SchemaError> {
        match self {
            AgentsTarget::V0(v) => v.check(),
            AgentsTarget::V1(v) => v.check(),
        }
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        Some(crate::types::check::union_answer(vec![crate::types::check::issues_of::<AgentsTargetV0>(value), crate::types::check::issues_of::<AgentsTargetV1>(value)]))
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        if path.is_empty() { return Some(crate::types::check::Zod::Union); }
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
        match obj.get("id") { None => out.push(crate::types::check::missing::<Self>("id")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("id"))), } } }
        match obj.get("name") { None => out.push(crate::types::check::missing::<Self>("name")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("name"))), } } }
        match obj.get("path") { None => out.push(crate::types::check::missing::<Self>("path")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("path"))), } } }
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
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum AgentRowSignIn {
    AgentSignInState(AgentSignInState),
    V1(UnknownLit),
}

impl Check for AgentRowSignIn {
    fn check(&self) -> Result<(), SchemaError> {
        match self {
            AgentRowSignIn::AgentSignInState(v) => v.check(),
            AgentRowSignIn::V1(v) => v.check(),
        }
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        Some(crate::types::check::union_answer(vec![crate::types::check::issues_of::<AgentSignInState>(value), crate::types::check::issues_of::<UnknownLit>(value)]))
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        if path.is_empty() { return Some(crate::types::check::Zod::Union); }
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
        match obj.get("to") { None => out.push(crate::types::check::missing::<Self>("to")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("to"))), } } }
        match obj.get("command") { None => out.push(crate::types::check::missing::<Self>("command")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("command"))), } } }
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
        match obj.get("id") { None => out.push(crate::types::check::missing::<Self>("id")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("id"))), } } }
        match obj.get("name") { None => out.push(crate::types::check::missing::<Self>("name")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("name"))), } } }
        match obj.get("installed") { None => out.push(crate::types::check::missing::<Self>("installed")), Some(v) => { match crate::types::check::read::<bool>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("installed"))), } } }
        if let Some(v) = obj.get("version") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("version"))), } }
        if let Some(v) = obj.get("latest") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("latest"))), } }
        if let Some(v) = obj.get("pinned") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("pinned"))), } }
        match obj.get("road") { None => out.push(crate::types::check::missing::<Self>("road")), Some(v) => { match crate::types::check::read::<AgentRoad>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("road"))), } } }
        if let Some(v) = obj.get("installsOnFirstRun") { match crate::types::check::read_present::<True>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("installsOnFirstRun"))), } }
        if let Some(v) = obj.get("versionUnread") { match crate::types::check::read_present::<True>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("versionUnread"))), } }
        if let Some(v) = obj.get("path") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("path"))), } }
        if let Some(v) = obj.get("via") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("via"))), } }
        match obj.get("signIn") { None => out.push(crate::types::check::missing::<Self>("signIn")), Some(v) => { match crate::types::check::read::<AgentRowSignIn>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("signIn"))), } } }
        match obj.get("signInRoad") { None => out.push(crate::types::check::missing::<Self>("signInRoad")), Some(v) => { match crate::types::check::read::<SignInRoad>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("signInRoad"))), } } }
        match obj.get("wspTools") { None => out.push(crate::types::check::missing::<Self>("wspTools")), Some(v) => { match crate::types::check::read::<bool>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("wspTools"))), } } }
        if let Some(v) = obj.get("signInDetail") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("signInDetail"))), } }
        if let Some(v) = obj.get("signInKind") { match crate::types::check::read_present::<AgentSignInKind>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("signInKind"))), } }
        if let Some(v) = obj.get("signInPlan") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("signInPlan"))), } }
        if let Some(v) = obj.get("update") { match crate::types::check::read_present::<AgentRowUpdate>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("update"))), } }
        if let Some(v) = obj.get("setup") { match crate::types::check::read_present::<AgentSetupView>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("setup"))), } }
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
        match obj.get("path") { None => out.push(crate::types::check::missing::<Self>("path")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("path"))), } } }
        if let Some(v) = obj.get("agent") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("agent"))), } }
        if let Some(v) = obj.get("linkTo") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("linkTo"))), } }
        if let Some(v) = obj.get("off") { match crate::types::check::read_present::<True>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("off"))), } }
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
        match obj.get("name") { None => out.push(crate::types::check::missing::<Self>("name")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("name"))), } } }
        if let Some(v) = obj.get("description") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("description"))), } }
        match obj.get("paths") { None => out.push(crate::types::check::missing::<Self>("paths")), Some(v) => { match crate::types::check::read::<Vec<SkillPath>>(v) { Ok(x) => { if let Err(e) = Self::rule_paths(&x) { out.push(e.under("paths")); } }, Err(es) => out.extend(es.into_iter().map(|e| e.under("paths"))), } } }
        match obj.get("scope") { None => out.push(crate::types::check::missing::<Self>("scope")), Some(v) => { match crate::types::check::read::<SkillScope>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("scope"))), } } }
        if let Some(v) = obj.get("project") { match crate::types::check::read_present::<AgentsProject>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("project"))), } }
        if let Some(v) = obj.get("plugin") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("plugin"))), } }
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
        crate::types::check::num_at_least(self.installs.0.0, 0.0).map_err(|e| e.under("installs"))?;
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        match obj.get("id") { None => out.push(crate::types::check::missing::<Self>("id")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("id"))), } } }
        match obj.get("source") { None => out.push(crate::types::check::missing::<Self>("source")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("source"))), } } }
        match obj.get("skillId") { None => out.push(crate::types::check::missing::<Self>("skillId")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("skillId"))), } } }
        match obj.get("name") { None => out.push(crate::types::check::missing::<Self>("name")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("name"))), } } }
        match obj.get("installs") { None => out.push(crate::types::check::missing::<Self>("installs")), Some(v) => { match crate::types::check::read::<JsInt>(v) { Ok(x) => { if let Err(e) = Self::rule_installs(&x) { out.push(e.under("installs")); } }, Err(es) => out.extend(es.into_iter().map(|e| e.under("installs"))), } } }
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
        crate::types::check::num_at_least((*x).0.0, 0.0)?;
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
        crate::types::check::num_at_least(self.size.0.0, 0.0).map_err(|e| e.under("size"))?;
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        match obj.get("text") { None => out.push(crate::types::check::missing::<Self>("text")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("text"))), } } }
        match obj.get("size") { None => out.push(crate::types::check::missing::<Self>("size")), Some(v) => { match crate::types::check::read::<JsInt>(v) { Ok(x) => { if let Err(e) = Self::rule_size(&x) { out.push(e.under("size")); } }, Err(es) => out.extend(es.into_iter().map(|e| e.under("size"))), } } }
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
        crate::types::check::num_at_least((*x).0.0, 0.0)?;
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
        match obj.get("agent") { None => out.push(crate::types::check::missing::<Self>("agent")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("agent"))), } } }
        match obj.get("path") { None => out.push(crate::types::check::missing::<Self>("path")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("path"))), } } }
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
        match obj.get("path") { None => out.push(crate::types::check::missing::<Self>("path")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("path"))), } } }
        match obj.get("agents") { None => out.push(crate::types::check::missing::<Self>("agents")), Some(v) => { match crate::types::check::read::<Vec<SkillAddedAgentsItem>>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("agents"))), } } }
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
        match obj.get("skills") { None => out.push(crate::types::check::missing::<Self>("skills")), Some(v) => { match crate::types::check::read::<Vec<String>>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("skills"))), } } }
        match obj.get("commands") { None => out.push(crate::types::check::missing::<Self>("commands")), Some(v) => { match crate::types::check::read::<Vec<String>>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("commands"))), } } }
        match obj.get("subagents") { None => out.push(crate::types::check::missing::<Self>("subagents")), Some(v) => { match crate::types::check::read::<Vec<String>>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("subagents"))), } } }
        match obj.get("hooks") { None => out.push(crate::types::check::missing::<Self>("hooks")), Some(v) => { match crate::types::check::read::<Vec<String>>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("hooks"))), } } }
        match obj.get("servers") { None => out.push(crate::types::check::missing::<Self>("servers")), Some(v) => { match crate::types::check::read::<Vec<String>>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("servers"))), } } }
        match obj.get("lsp") { None => out.push(crate::types::check::missing::<Self>("lsp")), Some(v) => { match crate::types::check::read::<Vec<String>>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("lsp"))), } } }
        match obj.get("apps") { None => out.push(crate::types::check::missing::<Self>("apps")), Some(v) => { match crate::types::check::read::<Vec<String>>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("apps"))), } } }
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

/// TS: packages/protocol/src/agents-report.ts:249 at a3b870a6e.
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
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub project: Option<AgentsProject>,
    pub on: bool,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub missing: Option<PluginMissing>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub set_in: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub source: Option<String>,
    pub brings: PluginBrings,
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
        match obj.get("agent") { None => out.push(crate::types::check::missing::<Self>("agent")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("agent"))), } } }
        match obj.get("id") { None => out.push(crate::types::check::missing::<Self>("id")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("id"))), } } }
        match obj.get("name") { None => out.push(crate::types::check::missing::<Self>("name")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("name"))), } } }
        match obj.get("marketplace") { None => out.push(crate::types::check::missing::<Self>("marketplace")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("marketplace"))), } } }
        if let Some(v) = obj.get("version") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("version"))), } }
        match obj.get("scope") { None => out.push(crate::types::check::missing::<Self>("scope")), Some(v) => { match crate::types::check::read::<PluginScope>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("scope"))), } } }
        if let Some(v) = obj.get("project") { match crate::types::check::read_present::<AgentsProject>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("project"))), } }
        match obj.get("on") { None => out.push(crate::types::check::missing::<Self>("on")), Some(v) => { match crate::types::check::read::<bool>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("on"))), } } }
        if let Some(v) = obj.get("missing") { match crate::types::check::read_present::<PluginMissing>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("missing"))), } }
        if let Some(v) = obj.get("path") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("path"))), } }
        if let Some(v) = obj.get("setIn") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("setIn"))), } }
        if let Some(v) = obj.get("description") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("description"))), } }
        if let Some(v) = obj.get("source") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("source"))), } }
        match obj.get("brings") { None => out.push(crate::types::check::missing::<Self>("brings")), Some(v) => { match crate::types::check::read::<PluginBrings>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("brings"))), } } }
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
        match obj.get("agent") { None => out.push(crate::types::check::missing::<Self>("agent")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("agent"))), } } }
        match obj.get("plugin") { None => out.push(crate::types::check::missing::<Self>("plugin")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("plugin"))), } } }
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
        match obj.get("name") { None => out.push(crate::types::check::missing::<Self>("name")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("name"))), } } }
        if let Some(v) = obj.get("type") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("type"))), } }
        match obj.get("required") { None => out.push(crate::types::check::missing::<Self>("required")), Some(v) => { match crate::types::check::read::<bool>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("required"))), } } }
        if let Some(v) = obj.get("description") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("description"))), } }
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
        match obj.get("name") { None => out.push(crate::types::check::missing::<Self>("name")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("name"))), } } }
        if let Some(v) = obj.get("description") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("description"))), } }
        if let Some(v) = obj.get("params") { match crate::types::check::read_present::<Vec<McpToolParam>>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("params"))), } }
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
        match obj.get("line") { None => out.push(crate::types::check::missing::<Self>("line")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("line"))), } } }
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
        match obj.get("host") { None => out.push(crate::types::check::missing::<Self>("host")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("host"))), } } }
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
        let Some(tag) = obj.get("kind") else { return Some(vec![crate::types::check::missing::<Self>("kind")]); };
        match tag.as_str() {
            Some("stdio") => { let mut body = obj.clone(); body.shift_remove("kind"); Some(crate::types::check::issues_of::<McpRowTransportStdio>(&serde_json::Value::Object(body))) }
            Some("http") => { let mut body = obj.clone(); body.shift_remove("kind"); Some(crate::types::check::issues_of::<McpRowTransportHttp>(&serde_json::Value::Object(body))) }
            _ => Some(vec![crate::types::check::unknown_tag::<Self>("kind", tag)]),
        }
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        if path.is_empty() { return Some(crate::types::check::Zod::Word("object")); }
        if path.len() == 1 && path[0].as_str() == Some("kind") { return Some(crate::types::check::Zod::Tags("kind", &["stdio", "http"])); }
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
        crate::types::check::leaf(path, crate::types::check::Zod::Options(&["open", "env-key", "connected", "signed-in", "needs-sign-in", "failed", "unknown"]))
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
        match obj.get("agent") { None => out.push(crate::types::check::missing::<Self>("agent")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("agent"))), } } }
        match obj.get("name") { None => out.push(crate::types::check::missing::<Self>("name")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("name"))), } } }
        if let Some(v) = obj.get("scope") { match crate::types::check::read_present::<McpScope>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("scope"))), } }
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
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present", serialize_with = "crate::types::check::js_record_opt")]
    pub env: Option<IndexMap<String, String>>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present", serialize_with = "crate::types::check::js_record_opt")]
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
        match obj.get("agent") { None => out.push(crate::types::check::missing::<Self>("agent")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("agent"))), } } }
        match obj.get("name") { None => out.push(crate::types::check::missing::<Self>("name")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("name"))), } } }
        if let Some(v) = obj.get("project") { match crate::types::check::read_present::<bool>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("project"))), } }
        if let Some(v) = obj.get("command") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("command"))), } }
        if let Some(v) = obj.get("args") { match crate::types::check::read_present::<Vec<String>>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("args"))), } }
        if let Some(v) = obj.get("env") { match crate::types::check::read_present::<IndexMap<String, String>>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("env"))), } }
        if let Some(v) = obj.get("url") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("url"))), } }
        if let Some(v) = obj.get("headers") { match crate::types::check::read_present::<IndexMap<String, String>>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("headers"))), } }
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
        match obj.get("agent") { None => out.push(crate::types::check::missing::<Self>("agent")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("agent"))), } } }
        match obj.get("name") { None => out.push(crate::types::check::missing::<Self>("name")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("name"))), } } }
        match obj.get("scope") { None => out.push(crate::types::check::missing::<Self>("scope")), Some(v) => { match crate::types::check::read::<McpScope>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("scope"))), } } }
        if let Some(v) = obj.get("file") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("file"))), } }
        if let Some(v) = obj.get("launch") { match crate::types::check::read_present::<True>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("launch"))), } }
        match obj.get("transport") { None => out.push(crate::types::check::missing::<Self>("transport")), Some(v) => { match crate::types::check::read::<McpRowTransport>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("transport"))), } } }
        match obj.get("envNames") { None => out.push(crate::types::check::missing::<Self>("envNames")), Some(v) => { match crate::types::check::read::<Vec<String>>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("envNames"))), } } }
        match obj.get("auth") { None => out.push(crate::types::check::missing::<Self>("auth")), Some(v) => { match crate::types::check::read::<McpAuth>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("auth"))), } } }
        match obj.get("enabled") { None => out.push(crate::types::check::missing::<Self>("enabled")), Some(v) => { match crate::types::check::read::<bool>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("enabled"))), } } }
        if let Some(v) = obj.get("inRecipe") { match crate::types::check::read_present::<bool>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("inRecipe"))), } }
        if let Some(v) = obj.get("tools") { match crate::types::check::read_present::<Vec<McpTool>>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("tools"))), } }
        if let Some(v) = obj.get("project") { match crate::types::check::read_present::<AgentsProject>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("project"))), } }
        if let Some(v) = obj.get("signInLine") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("signInLine"))), } }
        if let Some(v) = obj.get("off") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("off"))), } }
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
        match obj.get("auth") { None => out.push(crate::types::check::missing::<Self>("auth")), Some(v) => { match crate::types::check::read::<McpAuth>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("auth"))), } } }
        if let Some(v) = obj.get("tools") { match crate::types::check::read_present::<Vec<McpTool>>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("tools"))), } }
        if let Some(v) = obj.get("holder") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("holder"))), } }
        if let Some(v) = obj.get("refused") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("refused"))), } }
        match obj.get("readAt") { None => out.push(crate::types::check::missing::<Self>("readAt")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("readAt"))), } } }
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
        match obj.get("target") { None => out.push(crate::types::check::missing::<Self>("target")), Some(v) => { match crate::types::check::read::<AgentsTarget>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("target"))), } } }
        match obj.get("home") { None => out.push(crate::types::check::missing::<Self>("home")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("home"))), } } }
        match obj.get("user") { None => out.push(crate::types::check::missing::<Self>("user")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("user"))), } } }
        match obj.get("readAt") { None => out.push(crate::types::check::missing::<Self>("readAt")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("readAt"))), } } }
        if let Some(v) = obj.get("stale") { match crate::types::check::read_present::<NappingLit>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("stale"))), } }
        if let Some(v) = obj.get("pauseMode") { match crate::types::check::read_present::<PauseMode>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("pauseMode"))), } }
        match obj.get("agents") { None => out.push(crate::types::check::missing::<Self>("agents")), Some(v) => { match crate::types::check::read::<Vec<AgentRow>>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("agents"))), } } }
        match obj.get("skills") { None => out.push(crate::types::check::missing::<Self>("skills")), Some(v) => { match crate::types::check::read::<Vec<SkillRow>>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("skills"))), } } }
        match obj.get("servers") { None => out.push(crate::types::check::missing::<Self>("servers")), Some(v) => { match crate::types::check::read::<Vec<McpRow>>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("servers"))), } } }
        if let Some(v) = obj.get("plugins") { match crate::types::check::read_present::<Vec<PluginRow>>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("plugins"))), } }
        match obj.get("refused") { None => out.push(crate::types::check::missing::<Self>("refused")), Some(v) => { match crate::types::check::read::<Vec<String>>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("refused"))), } } }
        if let Some(v) = obj.get("projects") { match crate::types::check::read_present::<Vec<AgentsProject>>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("projects"))), } }
        if let Some(v) = obj.get("reach") { match crate::types::check::read_present::<PageReach>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("reach"))), } }
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
        match obj.get("type") { None => out.push(crate::types::check::missing::<Self>("type")), Some(v) => { match crate::types::check::read::<AgentsSignInEventType>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("type"))), } } }
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
        match obj.get("signInId") { None => out.push(crate::types::check::missing::<Self>("signInId")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("signInId"))), } } }
        match obj.get("state") { None => out.push(crate::types::check::missing::<Self>("state")), Some(v) => { match crate::types::check::read::<AgentsSignInEventState>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("state"))), } } }
        if let Some(v) = obj.get("url") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("url"))), } }
        if let Some(v) = obj.get("code") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("code"))), } }
        if let Some(v) = obj.get("paste") { match crate::types::check::read_present::<bool>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("paste"))), } }
        if let Some(v) = obj.get("said") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("said"))), } }
        if let Some(v) = obj.get("ptyId") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("ptyId"))), } }
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
        match obj.get("signInId") { None => out.push(crate::types::check::missing::<Self>("signInId")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("signInId"))), } } }
        match obj.get("target") { None => out.push(crate::types::check::missing::<Self>("target")), Some(v) => { match crate::types::check::read::<AgentsTarget>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("target"))), } } }
        match obj.get("agent") { None => out.push(crate::types::check::missing::<Self>("agent")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("agent"))), } } }
        if let Some(v) = obj.get("server") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("server"))), } }
        if let Some(v) = obj.get("scope") { match crate::types::check::read_present::<McpScope>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("scope"))), } }
        if let Some(v) = obj.get("project") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("project"))), } }
        if let Some(v) = obj.get("last") { match crate::types::check::read_present::<AgentsSignInRunLast>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("last"))), } }
        if let Some(v) = obj.get("ended") { match crate::types::check::read_present::<True>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("ended"))), } }
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
        if let Some(v) = obj.get("target") { match crate::types::check::read_present::<AgentsTarget>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("target"))), } }
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
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present", serialize_with = "crate::types::check::js_record_opt")]
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
        match obj.get("command") { None => out.push(crate::types::check::missing::<Self>("command")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("command"))), } } }
        if let Some(v) = obj.get("env") { match crate::types::check::read_present::<IndexMap<String, String>>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("env"))), } }
        if let Some(v) = obj.get("prepare") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("prepare"))), } }
        if let Some(v) = obj.get("status") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("status"))), } }
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
