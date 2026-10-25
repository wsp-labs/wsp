// SPDX-License-Identifier: AGPL-3.0-only
//! Port of packages/protocol/src/views/project-bundle.ts at a3b870a6e: its zod shapes and plain constants, written by the generator.

use serde::{Deserialize, Serialize};

use crate::types::check::{Check, SchemaError};
use crate::types::js_number::JsInt;
use crate::types::js_number::JsNumber;
use crate::views::workspace::WorkspaceProject;

/// TS: packages/protocol/src/views/project-bundle.ts:12 at a3b870a6e.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum CredentialSignal {
    #[serde(rename = "name")]
    Name,
    #[serde(rename = "mode")]
    Mode,
    #[serde(rename = "keys")]
    Keys,
    #[serde(rename = "pem")]
    Pem,
    #[serde(rename = "url")]
    Url,
    #[serde(rename = "gitleaks")]
    Gitleaks,
    #[serde(rename = "catalog")]
    Catalog,
    #[serde(rename = "exports")]
    Exports,
    #[serde(rename = "history")]
    History,
}

impl Check for CredentialSignal {
    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        crate::types::check::leaf(path, crate::types::check::Zod::Options(&["name", "mode", "keys", "pem", "url", "gitleaks", "catalog", "exports", "history"]))
    }
}

/// TS: packages/protocol/src/views/project-bundle.ts:16 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectRewrite {
    pub urls: Vec<String>,
    pub drop: Vec<String>,
}

impl Check for ProjectRewrite {
    fn check(&self) -> Result<(), SchemaError> {
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        match obj.get("urls") { None => out.push(crate::types::check::missing::<Self>("urls")), Some(v) => { match crate::types::check::read::<Vec<String>>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("urls"))), } } }
        match obj.get("drop") { None => out.push(crate::types::check::missing::<Self>("drop")), Some(v) => { match crate::types::check::read::<Vec<String>>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("drop"))), } } }
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "urls" => <Vec<String> as Check>::describe(rest),
                "drop" => <Vec<String> as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

/// TS: packages/protocol/src/views/project-bundle.ts:21 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectSecret {
    pub path: String,
    pub bytes: JsNumber,
    pub signals: Vec<CredentialSignal>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub rewrite: Option<ProjectRewrite>,
}

impl Check for ProjectSecret {
    /// Nested checks: signals, rewrite.
    fn check(&self) -> Result<(), SchemaError> {
        self.signals.check().map_err(|e| e.under("signals"))?;
        self.rewrite.check().map_err(|e| e.under("rewrite"))?;
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        match obj.get("path") { None => out.push(crate::types::check::missing::<Self>("path")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("path"))), } } }
        match obj.get("bytes") { None => out.push(crate::types::check::missing::<Self>("bytes")), Some(v) => { match crate::types::check::read::<JsNumber>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("bytes"))), } } }
        match obj.get("signals") { None => out.push(crate::types::check::missing::<Self>("signals")), Some(v) => { match crate::types::check::read::<Vec<CredentialSignal>>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("signals"))), } } }
        if let Some(v) = obj.get("rewrite") { match crate::types::check::read_present::<ProjectRewrite>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("rewrite"))), } }
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "path" => <String as Check>::describe(rest),
                "bytes" => <JsNumber as Check>::describe(rest),
                "signals" => <Vec<CredentialSignal> as Check>::describe(rest),
                "rewrite" => <Option<ProjectRewrite> as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

/// TS: packages/protocol/src/views/project-bundle.ts:26 at a3b870a6e.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ProjectCarry {
    #[serde(rename = "moves")]
    Moves,
    #[serde(rename = "transcript-only")]
    TranscriptOnly,
}

impl Check for ProjectCarry {
    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        crate::types::check::leaf(path, crate::types::check::Zod::Options(&["moves", "transcript-only"]))
    }
}

/// TS: packages/protocol/src/views/project-bundle.ts:31 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectAgent {
    pub agent: String,
    pub name: String,
    pub sessions: JsNumber,
    pub bytes: JsNumber,
    pub carry: ProjectCarry,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub error: Option<String>,
}

impl Check for ProjectAgent {
    /// Nested checks: carry.
    fn check(&self) -> Result<(), SchemaError> {
        self.carry.check().map_err(|e| e.under("carry"))?;
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        match obj.get("agent") { None => out.push(crate::types::check::missing::<Self>("agent")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("agent"))), } } }
        match obj.get("name") { None => out.push(crate::types::check::missing::<Self>("name")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("name"))), } } }
        match obj.get("sessions") { None => out.push(crate::types::check::missing::<Self>("sessions")), Some(v) => { match crate::types::check::read::<JsNumber>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("sessions"))), } } }
        match obj.get("bytes") { None => out.push(crate::types::check::missing::<Self>("bytes")), Some(v) => { match crate::types::check::read::<JsNumber>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("bytes"))), } } }
        match obj.get("carry") { None => out.push(crate::types::check::missing::<Self>("carry")), Some(v) => { match crate::types::check::read::<ProjectCarry>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("carry"))), } } }
        if let Some(v) = obj.get("error") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("error"))), } }
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "agent" => <String as Check>::describe(rest),
                "name" => <String as Check>::describe(rest),
                "sessions" => <JsNumber as Check>::describe(rest),
                "bytes" => <JsNumber as Check>::describe(rest),
                "carry" => <ProjectCarry as Check>::describe(rest),
                "error" => <Option<String> as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectPlanSkippedItem {
    pub path: String,
    pub note: String,
}

impl Check for ProjectPlanSkippedItem {
    fn check(&self) -> Result<(), SchemaError> {
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        match obj.get("path") { None => out.push(crate::types::check::missing::<Self>("path")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("path"))), } } }
        match obj.get("note") { None => out.push(crate::types::check::missing::<Self>("note")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("note"))), } } }
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "path" => <String as Check>::describe(rest),
                "note" => <String as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

/// TS: packages/protocol/src/views/project-bundle.ts:36 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectPlan {
    pub source: String,
    pub repo: bool,
    pub files: JsNumber,
    pub bytes: JsNumber,
    pub secrets: Vec<ProjectSecret>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub values: Option<Vec<String>>,
    pub excluded: Vec<String>,
    pub skipped: Vec<ProjectPlanSkippedItem>,
    pub agents: Vec<ProjectAgent>,
}

impl Check for ProjectPlan {
    /// Nested checks: secrets, skipped, agents.
    fn check(&self) -> Result<(), SchemaError> {
        self.secrets.check().map_err(|e| e.under("secrets"))?;
        self.skipped.check().map_err(|e| e.under("skipped"))?;
        self.agents.check().map_err(|e| e.under("agents"))?;
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        match obj.get("source") { None => out.push(crate::types::check::missing::<Self>("source")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("source"))), } } }
        match obj.get("repo") { None => out.push(crate::types::check::missing::<Self>("repo")), Some(v) => { match crate::types::check::read::<bool>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("repo"))), } } }
        match obj.get("files") { None => out.push(crate::types::check::missing::<Self>("files")), Some(v) => { match crate::types::check::read::<JsNumber>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("files"))), } } }
        match obj.get("bytes") { None => out.push(crate::types::check::missing::<Self>("bytes")), Some(v) => { match crate::types::check::read::<JsNumber>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("bytes"))), } } }
        match obj.get("secrets") { None => out.push(crate::types::check::missing::<Self>("secrets")), Some(v) => { match crate::types::check::read::<Vec<ProjectSecret>>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("secrets"))), } } }
        if let Some(v) = obj.get("values") { match crate::types::check::read_present::<Vec<String>>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("values"))), } }
        match obj.get("excluded") { None => out.push(crate::types::check::missing::<Self>("excluded")), Some(v) => { match crate::types::check::read::<Vec<String>>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("excluded"))), } } }
        match obj.get("skipped") { None => out.push(crate::types::check::missing::<Self>("skipped")), Some(v) => { match crate::types::check::read::<Vec<ProjectPlanSkippedItem>>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("skipped"))), } } }
        match obj.get("agents") { None => out.push(crate::types::check::missing::<Self>("agents")), Some(v) => { match crate::types::check::read::<Vec<ProjectAgent>>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("agents"))), } } }
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "source" => <String as Check>::describe(rest),
                "repo" => <bool as Check>::describe(rest),
                "files" => <JsNumber as Check>::describe(rest),
                "bytes" => <JsNumber as Check>::describe(rest),
                "secrets" => <Vec<ProjectSecret> as Check>::describe(rest),
                "values" => <Option<Vec<String>> as Check>::describe(rest),
                "excluded" => <Vec<String> as Check>::describe(rest),
                "skipped" => <Vec<ProjectPlanSkippedItem> as Check>::describe(rest),
                "agents" => <Vec<ProjectAgent> as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

/// TS: packages/protocol/src/views/project-bundle.ts:53 at a3b870a6e.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ProjectImportStage {
    #[serde(rename = "planned")]
    Planned,
    #[serde(rename = "consented")]
    Consented,
    #[serde(rename = "packing")]
    Packing,
    #[serde(rename = "uploading")]
    Uploading,
    #[serde(rename = "landing")]
    Landing,
    #[serde(rename = "done")]
    Done,
    #[serde(rename = "failed")]
    Failed,
}

impl Check for ProjectImportStage {
    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        crate::types::check::leaf(path, crate::types::check::Zod::Options(&["planned", "consented", "packing", "uploading", "landing", "done", "failed"]))
    }
}

/// TS: packages/protocol/src/views/project-bundle.ts:57 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectImportEvent {
    pub workspace_id: String,
    pub source: String,
    pub dest: String,
    pub stage: ProjectImportStage,
    pub message: String,
    pub elapsed_ms: JsNumber,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub bytes: Option<JsNumber>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub total: Option<JsNumber>,
}

impl ProjectImportEvent {
    /// The `type` key's value: the enum holding this struct writes and reads it as its tag.
    pub const TYPE: &'static str = "project.import";
}

impl Check for ProjectImportEvent {
    /// Nested checks: stage.
    fn check(&self) -> Result<(), SchemaError> {
        self.stage.check().map_err(|e| e.under("stage"))?;
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        match obj.get("workspaceId") { None => out.push(crate::types::check::missing::<Self>("workspaceId")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("workspaceId"))), } } }
        match obj.get("source") { None => out.push(crate::types::check::missing::<Self>("source")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("source"))), } } }
        match obj.get("dest") { None => out.push(crate::types::check::missing::<Self>("dest")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("dest"))), } } }
        match obj.get("stage") { None => out.push(crate::types::check::missing::<Self>("stage")), Some(v) => { match crate::types::check::read::<ProjectImportStage>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("stage"))), } } }
        match obj.get("message") { None => out.push(crate::types::check::missing::<Self>("message")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("message"))), } } }
        match obj.get("elapsedMs") { None => out.push(crate::types::check::missing::<Self>("elapsedMs")), Some(v) => { match crate::types::check::read::<JsNumber>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("elapsedMs"))), } } }
        if let Some(v) = obj.get("bytes") { match crate::types::check::read_present::<JsNumber>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("bytes"))), } }
        if let Some(v) = obj.get("total") { match crate::types::check::read_present::<JsNumber>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("total"))), } }
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "workspaceId" => <String as Check>::describe(rest),
                "source" => <String as Check>::describe(rest),
                "dest" => <String as Check>::describe(rest),
                "stage" => <ProjectImportStage as Check>::describe(rest),
                "message" => <String as Check>::describe(rest),
                "elapsedMs" => <JsNumber as Check>::describe(rest),
                "bytes" => <Option<JsNumber> as Check>::describe(rest),
                "total" => <Option<JsNumber> as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

/// TS: packages/protocol/src/views/project-bundle.ts:74 at a3b870a6e.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ProjectAgentOutcome {
    #[serde(rename = "moved")]
    Moved,
    #[serde(rename = "transcript-only")]
    TranscriptOnly,
    #[serde(rename = "carried")]
    Carried,
    #[serde(rename = "nothing")]
    Nothing,
    #[serde(rename = "failed")]
    Failed,
}

impl Check for ProjectAgentOutcome {
    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        crate::types::check::leaf(path, crate::types::check::Zod::Options(&["moved", "transcript-only", "carried", "nothing", "failed"]))
    }
}

/// TS: packages/protocol/src/views/project-bundle.ts:81 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectAgentResult {
    pub agent: String,
    pub files: JsNumber,
    pub bytes: JsNumber,
    pub outcome: ProjectAgentOutcome,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub sessions: Option<JsNumber>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub skipped: Option<JsNumber>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub error: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub rows: Option<JsNumber>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub note: Option<String>,
}

impl Check for ProjectAgentResult {
    /// Nested checks: outcome.
    fn check(&self) -> Result<(), SchemaError> {
        self.outcome.check().map_err(|e| e.under("outcome"))?;
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        match obj.get("agent") { None => out.push(crate::types::check::missing::<Self>("agent")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("agent"))), } } }
        match obj.get("files") { None => out.push(crate::types::check::missing::<Self>("files")), Some(v) => { match crate::types::check::read::<JsNumber>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("files"))), } } }
        match obj.get("bytes") { None => out.push(crate::types::check::missing::<Self>("bytes")), Some(v) => { match crate::types::check::read::<JsNumber>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("bytes"))), } } }
        match obj.get("outcome") { None => out.push(crate::types::check::missing::<Self>("outcome")), Some(v) => { match crate::types::check::read::<ProjectAgentOutcome>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("outcome"))), } } }
        if let Some(v) = obj.get("sessions") { match crate::types::check::read_present::<JsNumber>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("sessions"))), } }
        if let Some(v) = obj.get("skipped") { match crate::types::check::read_present::<JsNumber>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("skipped"))), } }
        if let Some(v) = obj.get("error") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("error"))), } }
        if let Some(v) = obj.get("rows") { match crate::types::check::read_present::<JsNumber>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("rows"))), } }
        if let Some(v) = obj.get("note") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("note"))), } }
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "agent" => <String as Check>::describe(rest),
                "files" => <JsNumber as Check>::describe(rest),
                "bytes" => <JsNumber as Check>::describe(rest),
                "outcome" => <ProjectAgentOutcome as Check>::describe(rest),
                "sessions" => <Option<JsNumber> as Check>::describe(rest),
                "skipped" => <Option<JsNumber> as Check>::describe(rest),
                "error" => <Option<String> as Check>::describe(rest),
                "rows" => <Option<JsNumber> as Check>::describe(rest),
                "note" => <Option<String> as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

/// TS: packages/protocol/src/views/project-bundle.ts:97 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectImportResult {
    pub dest: String,
    pub files: JsNumber,
    pub bytes: JsNumber,
    pub parts: JsNumber,
    pub cut: Vec<String>,
    pub rewritten: Vec<String>,
    pub agents: Vec<ProjectAgentResult>,
    pub project: WorkspaceProject,
}

impl Check for ProjectImportResult {
    /// Nested checks: agents, project.
    fn check(&self) -> Result<(), SchemaError> {
        self.agents.check().map_err(|e| e.under("agents"))?;
        self.project.check().map_err(|e| e.under("project"))?;
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        match obj.get("dest") { None => out.push(crate::types::check::missing::<Self>("dest")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("dest"))), } } }
        match obj.get("files") { None => out.push(crate::types::check::missing::<Self>("files")), Some(v) => { match crate::types::check::read::<JsNumber>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("files"))), } } }
        match obj.get("bytes") { None => out.push(crate::types::check::missing::<Self>("bytes")), Some(v) => { match crate::types::check::read::<JsNumber>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("bytes"))), } } }
        match obj.get("parts") { None => out.push(crate::types::check::missing::<Self>("parts")), Some(v) => { match crate::types::check::read::<JsNumber>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("parts"))), } } }
        match obj.get("cut") { None => out.push(crate::types::check::missing::<Self>("cut")), Some(v) => { match crate::types::check::read::<Vec<String>>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("cut"))), } } }
        match obj.get("rewritten") { None => out.push(crate::types::check::missing::<Self>("rewritten")), Some(v) => { match crate::types::check::read::<Vec<String>>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("rewritten"))), } } }
        match obj.get("agents") { None => out.push(crate::types::check::missing::<Self>("agents")), Some(v) => { match crate::types::check::read::<Vec<ProjectAgentResult>>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("agents"))), } } }
        match obj.get("project") { None => out.push(crate::types::check::missing::<Self>("project")), Some(v) => { match crate::types::check::read::<WorkspaceProject>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("project"))), } } }
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "dest" => <String as Check>::describe(rest),
                "files" => <JsNumber as Check>::describe(rest),
                "bytes" => <JsNumber as Check>::describe(rest),
                "parts" => <JsNumber as Check>::describe(rest),
                "cut" => <Vec<String> as Check>::describe(rest),
                "rewritten" => <Vec<String> as Check>::describe(rest),
                "agents" => <Vec<ProjectAgentResult> as Check>::describe(rest),
                "project" => <WorkspaceProject as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

/// TS: packages/protocol/src/views/project-bundle.ts:100 at a3b870a6e.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ProjectExportStage {
    #[serde(rename = "packing")]
    Packing,
    #[serde(rename = "downloading")]
    Downloading,
    #[serde(rename = "landing")]
    Landing,
    #[serde(rename = "done")]
    Done,
    #[serde(rename = "failed")]
    Failed,
}

impl Check for ProjectExportStage {
    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        crate::types::check::leaf(path, crate::types::check::Zod::Options(&["packing", "downloading", "landing", "done", "failed"]))
    }
}

/// TS: packages/protocol/src/views/project-bundle.ts:105 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectExportEvent {
    pub workspace_id: String,
    pub source: String,
    pub dest: String,
    pub stage: ProjectExportStage,
    pub message: String,
    pub elapsed_ms: JsNumber,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub bytes: Option<JsNumber>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub total: Option<JsNumber>,
}

impl ProjectExportEvent {
    /// The `type` key's value: the enum holding this struct writes and reads it as its tag.
    pub const TYPE: &'static str = "project.export";
}

impl Check for ProjectExportEvent {
    /// Nested checks: stage.
    fn check(&self) -> Result<(), SchemaError> {
        self.stage.check().map_err(|e| e.under("stage"))?;
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        match obj.get("workspaceId") { None => out.push(crate::types::check::missing::<Self>("workspaceId")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("workspaceId"))), } } }
        match obj.get("source") { None => out.push(crate::types::check::missing::<Self>("source")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("source"))), } } }
        match obj.get("dest") { None => out.push(crate::types::check::missing::<Self>("dest")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("dest"))), } } }
        match obj.get("stage") { None => out.push(crate::types::check::missing::<Self>("stage")), Some(v) => { match crate::types::check::read::<ProjectExportStage>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("stage"))), } } }
        match obj.get("message") { None => out.push(crate::types::check::missing::<Self>("message")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("message"))), } } }
        match obj.get("elapsedMs") { None => out.push(crate::types::check::missing::<Self>("elapsedMs")), Some(v) => { match crate::types::check::read::<JsNumber>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("elapsedMs"))), } } }
        if let Some(v) = obj.get("bytes") { match crate::types::check::read_present::<JsNumber>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("bytes"))), } }
        if let Some(v) = obj.get("total") { match crate::types::check::read_present::<JsNumber>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("total"))), } }
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "workspaceId" => <String as Check>::describe(rest),
                "source" => <String as Check>::describe(rest),
                "dest" => <String as Check>::describe(rest),
                "stage" => <ProjectExportStage as Check>::describe(rest),
                "message" => <String as Check>::describe(rest),
                "elapsedMs" => <JsNumber as Check>::describe(rest),
                "bytes" => <Option<JsNumber> as Check>::describe(rest),
                "total" => <Option<JsNumber> as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

/// TS: packages/protocol/src/views/project-bundle.ts:121 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectExportResult {
    pub dest: String,
    pub files: JsNumber,
    pub bytes: JsNumber,
    pub excluded: Vec<String>,
    pub agents: Vec<ProjectAgentResult>,
}

impl Check for ProjectExportResult {
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
        match obj.get("dest") { None => out.push(crate::types::check::missing::<Self>("dest")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("dest"))), } } }
        match obj.get("files") { None => out.push(crate::types::check::missing::<Self>("files")), Some(v) => { match crate::types::check::read::<JsNumber>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("files"))), } } }
        match obj.get("bytes") { None => out.push(crate::types::check::missing::<Self>("bytes")), Some(v) => { match crate::types::check::read::<JsNumber>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("bytes"))), } } }
        match obj.get("excluded") { None => out.push(crate::types::check::missing::<Self>("excluded")), Some(v) => { match crate::types::check::read::<Vec<String>>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("excluded"))), } } }
        match obj.get("agents") { None => out.push(crate::types::check::missing::<Self>("agents")), Some(v) => { match crate::types::check::read::<Vec<ProjectAgentResult>>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("agents"))), } } }
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "dest" => <String as Check>::describe(rest),
                "files" => <JsNumber as Check>::describe(rest),
                "bytes" => <JsNumber as Check>::describe(rest),
                "excluded" => <Vec<String> as Check>::describe(rest),
                "agents" => <Vec<ProjectAgentResult> as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

/// TS: packages/protocol/src/views/project-bundle.ts:127 at a3b870a6e.
/// A folder on the host a person may add a project from; touchedAt is a plain number in zod.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HostFolder {
    pub path: String,
    pub repo: bool,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub branch: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub touched_at: Option<JsNumber>,
}

impl Check for HostFolder {
    fn check(&self) -> Result<(), SchemaError> {
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        match obj.get("path") { None => out.push(crate::types::check::missing::<Self>("path")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("path"))), } } }
        match obj.get("repo") { None => out.push(crate::types::check::missing::<Self>("repo")), Some(v) => { match crate::types::check::read::<bool>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("repo"))), } } }
        if let Some(v) = obj.get("branch") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("branch"))), } }
        if let Some(v) = obj.get("touchedAt") { match crate::types::check::read_present::<JsNumber>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("touchedAt"))), } }
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "path" => <String as Check>::describe(rest),
                "repo" => <bool as Check>::describe(rest),
                "branch" => <Option<String> as Check>::describe(rest),
                "touchedAt" => <Option<JsNumber> as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

/// TS: packages/protocol/src/views/project-bundle.ts:141 at a3b870a6e.
/// A listing of the host's folders under a directory.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HostFolderListing {
    pub dir: String,
    pub roots: Vec<String>,
    pub folders: Vec<HostFolder>,
    pub hidden: JsInt,
}

impl Check for HostFolderListing {
    fn check(&self) -> Result<(), SchemaError> {
        self.folders.check().map_err(|e| e.under("folders"))?;
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        match obj.get("dir") { None => out.push(crate::types::check::missing::<Self>("dir")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("dir"))), } } }
        match obj.get("roots") { None => out.push(crate::types::check::missing::<Self>("roots")), Some(v) => { match crate::types::check::read::<Vec<String>>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("roots"))), } } }
        match obj.get("folders") { None => out.push(crate::types::check::missing::<Self>("folders")), Some(v) => { match crate::types::check::read::<Vec<HostFolder>>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("folders"))), } } }
        match obj.get("hidden") { None => out.push(crate::types::check::missing::<Self>("hidden")), Some(v) => { match crate::types::check::read::<JsInt>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("hidden"))), } } }
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "dir" => <String as Check>::describe(rest),
                "roots" => <Vec<String> as Check>::describe(rest),
                "folders" => <Vec<HostFolder> as Check>::describe(rest),
                "hidden" => <i64 as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

/// TS: packages/protocol/src/views/project-bundle.ts:151 at a3b870a6e.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TerminalScheme {
    #[serde(rename = "light")]
    Light,
    #[serde(rename = "dark")]
    Dark,
}

impl Check for TerminalScheme {
    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        crate::types::check::leaf(path, crate::types::check::Zod::Options(&["light", "dark"]))
    }
}

/// TS: packages/protocol/src/views/project-bundle.ts:154 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TerminalRgb {
    pub r: JsInt,
    pub g: JsInt,
    pub b: JsInt,
}

impl Check for TerminalRgb {
    /// Rules: r: >= 0; r: <= 255; g: >= 0; g: <= 255; b: >= 0; b: <= 255.
    fn check(&self) -> Result<(), SchemaError> {
        crate::types::check::num_at_least(self.r.0.0, 0.0).map_err(|e| e.under("r"))?;
        crate::types::check::num_at_most(self.r.0.0, 255.0).map_err(|e| e.under("r"))?;
        crate::types::check::num_at_least(self.g.0.0, 0.0).map_err(|e| e.under("g"))?;
        crate::types::check::num_at_most(self.g.0.0, 255.0).map_err(|e| e.under("g"))?;
        crate::types::check::num_at_least(self.b.0.0, 0.0).map_err(|e| e.under("b"))?;
        crate::types::check::num_at_most(self.b.0.0, 255.0).map_err(|e| e.under("b"))?;
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        match obj.get("r") { None => out.push(crate::types::check::missing::<Self>("r")), Some(v) => { match crate::types::check::read::<JsInt>(v) { Ok(x) => { if let Err(e) = Self::rule_r(&x) { out.push(e.under("r")); } }, Err(es) => out.extend(es.into_iter().map(|e| e.under("r"))), } } }
        match obj.get("g") { None => out.push(crate::types::check::missing::<Self>("g")), Some(v) => { match crate::types::check::read::<JsInt>(v) { Ok(x) => { if let Err(e) = Self::rule_g(&x) { out.push(e.under("g")); } }, Err(es) => out.extend(es.into_iter().map(|e| e.under("g"))), } } }
        match obj.get("b") { None => out.push(crate::types::check::missing::<Self>("b")), Some(v) => { match crate::types::check::read::<JsInt>(v) { Ok(x) => { if let Err(e) = Self::rule_b(&x) { out.push(e.under("b")); } }, Err(es) => out.extend(es.into_iter().map(|e| e.under("b"))), } } }
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "r" => <i64 as Check>::describe(rest),
                "g" => <i64 as Check>::describe(rest),
                "b" => <i64 as Check>::describe(rest),
                _ => None,
            },
        }
    }
}
impl TerminalRgb {
    fn rule_r(x: &JsInt) -> Result<(), crate::types::check::SchemaError> {
        crate::types::check::num_at_least((*x).0.0, 0.0)?;
        crate::types::check::num_at_most((*x).0.0, 255.0)?;
        Ok(())
    }
    fn rule_g(x: &JsInt) -> Result<(), crate::types::check::SchemaError> {
        crate::types::check::num_at_least((*x).0.0, 0.0)?;
        crate::types::check::num_at_most((*x).0.0, 255.0)?;
        Ok(())
    }
    fn rule_b(x: &JsInt) -> Result<(), crate::types::check::SchemaError> {
        crate::types::check::num_at_least((*x).0.0, 0.0)?;
        crate::types::check::num_at_most((*x).0.0, 255.0)?;
        Ok(())
    }
}

/// TS: packages/protocol/src/views/project-bundle.ts:157 at a3b870a6e.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TerminalCursorStyle {
    #[serde(rename = "block")]
    Block,
    #[serde(rename = "bar")]
    Bar,
    #[serde(rename = "underline")]
    Underline,
    #[serde(rename = "block_hollow")]
    BlockHollow,
}

impl Check for TerminalCursorStyle {
    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        crate::types::check::leaf(path, crate::types::check::Zod::Options(&["block", "bar", "underline", "block_hollow"]))
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TerminalConfigWindowPaddingX {
    pub left: JsNumber,
    pub right: JsNumber,
}

impl Check for TerminalConfigWindowPaddingX {
    /// Rules: left: >= 0; right: >= 0.
    fn check(&self) -> Result<(), SchemaError> {
        crate::types::check::num_at_least(self.left.0, 0.0).map_err(|e| e.under("left"))?;
        crate::types::check::num_at_least(self.right.0, 0.0).map_err(|e| e.under("right"))?;
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        match obj.get("left") { None => out.push(crate::types::check::missing::<Self>("left")), Some(v) => { match crate::types::check::read::<JsNumber>(v) { Ok(x) => { if let Err(e) = Self::rule_left(&x) { out.push(e.under("left")); } }, Err(es) => out.extend(es.into_iter().map(|e| e.under("left"))), } } }
        match obj.get("right") { None => out.push(crate::types::check::missing::<Self>("right")), Some(v) => { match crate::types::check::read::<JsNumber>(v) { Ok(x) => { if let Err(e) = Self::rule_right(&x) { out.push(e.under("right")); } }, Err(es) => out.extend(es.into_iter().map(|e| e.under("right"))), } } }
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "left" => <JsNumber as Check>::describe(rest),
                "right" => <JsNumber as Check>::describe(rest),
                _ => None,
            },
        }
    }
}
impl TerminalConfigWindowPaddingX {
    fn rule_left(x: &JsNumber) -> Result<(), crate::types::check::SchemaError> {
        crate::types::check::num_at_least((*x).0, 0.0)?;
        Ok(())
    }
    fn rule_right(x: &JsNumber) -> Result<(), crate::types::check::SchemaError> {
        crate::types::check::num_at_least((*x).0, 0.0)?;
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TerminalConfigWindowPaddingY {
    pub top: JsNumber,
    pub bottom: JsNumber,
}

impl Check for TerminalConfigWindowPaddingY {
    /// Rules: top: >= 0; bottom: >= 0.
    fn check(&self) -> Result<(), SchemaError> {
        crate::types::check::num_at_least(self.top.0, 0.0).map_err(|e| e.under("top"))?;
        crate::types::check::num_at_least(self.bottom.0, 0.0).map_err(|e| e.under("bottom"))?;
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        match obj.get("top") { None => out.push(crate::types::check::missing::<Self>("top")), Some(v) => { match crate::types::check::read::<JsNumber>(v) { Ok(x) => { if let Err(e) = Self::rule_top(&x) { out.push(e.under("top")); } }, Err(es) => out.extend(es.into_iter().map(|e| e.under("top"))), } } }
        match obj.get("bottom") { None => out.push(crate::types::check::missing::<Self>("bottom")), Some(v) => { match crate::types::check::read::<JsNumber>(v) { Ok(x) => { if let Err(e) = Self::rule_bottom(&x) { out.push(e.under("bottom")); } }, Err(es) => out.extend(es.into_iter().map(|e| e.under("bottom"))), } } }
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "top" => <JsNumber as Check>::describe(rest),
                "bottom" => <JsNumber as Check>::describe(rest),
                _ => None,
            },
        }
    }
}
impl TerminalConfigWindowPaddingY {
    fn rule_top(x: &JsNumber) -> Result<(), crate::types::check::SchemaError> {
        crate::types::check::num_at_least((*x).0, 0.0)?;
        Ok(())
    }
    fn rule_bottom(x: &JsNumber) -> Result<(), crate::types::check::SchemaError> {
        crate::types::check::num_at_least((*x).0, 0.0)?;
        Ok(())
    }
}

/// TS: packages/protocol/src/views/project-bundle.ts:164 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TerminalConfig {
    pub files: Vec<String>,
    pub font_family: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub font_size: Option<JsNumber>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub theme: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub background: Option<TerminalRgb>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub foreground: Option<TerminalRgb>,
    pub palette: Vec<Option<TerminalRgb>>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub selection_background: Option<TerminalRgb>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub cursor_color: Option<TerminalRgb>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub cursor_style: Option<TerminalCursorStyle>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub cursor_style_blink: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub window_padding_x: Option<TerminalConfigWindowPaddingX>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub window_padding_y: Option<TerminalConfigWindowPaddingY>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub background_opacity: Option<JsNumber>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub background_blur: Option<JsInt>,
}

impl Check for TerminalConfig {
    /// Rules: fontSize: > 0; palette: exactly 16 items; backgroundOpacity: >= 0; backgroundOpacity: <= 1; backgroundBlur: >= 0.
    /// Nested checks: background, foreground, palette, selectionBackground, cursorColor, cursorStyle, windowPaddingX, windowPaddingY.
    fn check(&self) -> Result<(), SchemaError> {
        if let Some(v) = self.font_size {
            crate::types::check::num_above(v.0, 0.0).map_err(|e| e.under("fontSize"))?;
        }
        self.background.check().map_err(|e| e.under("background"))?;
        self.foreground.check().map_err(|e| e.under("foreground"))?;
        crate::types::check::items_exactly(self.palette.len(), 16).map_err(|e| e.under("palette"))?;
        self.palette.check().map_err(|e| e.under("palette"))?;
        self.selection_background.check().map_err(|e| e.under("selectionBackground"))?;
        self.cursor_color.check().map_err(|e| e.under("cursorColor"))?;
        self.cursor_style.check().map_err(|e| e.under("cursorStyle"))?;
        self.window_padding_x.check().map_err(|e| e.under("windowPaddingX"))?;
        self.window_padding_y.check().map_err(|e| e.under("windowPaddingY"))?;
        if let Some(v) = self.background_opacity {
            crate::types::check::num_at_least(v.0, 0.0).map_err(|e| e.under("backgroundOpacity"))?;
        }
        if let Some(v) = self.background_opacity {
            crate::types::check::num_at_most(v.0, 1.0).map_err(|e| e.under("backgroundOpacity"))?;
        }
        if let Some(v) = self.background_blur {
            crate::types::check::num_at_least(v.0.0, 0.0).map_err(|e| e.under("backgroundBlur"))?;
        }
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        match obj.get("files") { None => out.push(crate::types::check::missing::<Self>("files")), Some(v) => { match crate::types::check::read::<Vec<String>>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("files"))), } } }
        match obj.get("fontFamily") { None => out.push(crate::types::check::missing::<Self>("fontFamily")), Some(v) => { match crate::types::check::read::<Vec<String>>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("fontFamily"))), } } }
        if let Some(v) = obj.get("fontSize") { match crate::types::check::read_present::<JsNumber>(v) { Ok(x) => { if let Err(e) = Self::rule_font_size(&x) { out.push(e.under("fontSize")); } }, Err(es) => out.extend(es.into_iter().map(|e| e.under("fontSize"))), } }
        if let Some(v) = obj.get("theme") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("theme"))), } }
        if let Some(v) = obj.get("background") { match crate::types::check::read_present::<TerminalRgb>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("background"))), } }
        if let Some(v) = obj.get("foreground") { match crate::types::check::read_present::<TerminalRgb>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("foreground"))), } }
        match obj.get("palette") { None => out.push(crate::types::check::missing::<Self>("palette")), Some(v) => { match crate::types::check::read::<Vec<Option<TerminalRgb>>>(v) { Ok(x) => { if let Err(e) = Self::rule_palette(&x) { out.push(e.under("palette")); } }, Err(es) => out.extend(es.into_iter().map(|e| e.under("palette"))), } } }
        if let Some(v) = obj.get("selectionBackground") { match crate::types::check::read_present::<TerminalRgb>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("selectionBackground"))), } }
        if let Some(v) = obj.get("cursorColor") { match crate::types::check::read_present::<TerminalRgb>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("cursorColor"))), } }
        if let Some(v) = obj.get("cursorStyle") { match crate::types::check::read_present::<TerminalCursorStyle>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("cursorStyle"))), } }
        if let Some(v) = obj.get("cursorStyleBlink") { match crate::types::check::read_present::<bool>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("cursorStyleBlink"))), } }
        if let Some(v) = obj.get("windowPaddingX") { match crate::types::check::read_present::<TerminalConfigWindowPaddingX>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("windowPaddingX"))), } }
        if let Some(v) = obj.get("windowPaddingY") { match crate::types::check::read_present::<TerminalConfigWindowPaddingY>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("windowPaddingY"))), } }
        if let Some(v) = obj.get("backgroundOpacity") { match crate::types::check::read_present::<JsNumber>(v) { Ok(x) => { if let Err(e) = Self::rule_background_opacity(&x) { out.push(e.under("backgroundOpacity")); } }, Err(es) => out.extend(es.into_iter().map(|e| e.under("backgroundOpacity"))), } }
        if let Some(v) = obj.get("backgroundBlur") { match crate::types::check::read_present::<JsInt>(v) { Ok(x) => { if let Err(e) = Self::rule_background_blur(&x) { out.push(e.under("backgroundBlur")); } }, Err(es) => out.extend(es.into_iter().map(|e| e.under("backgroundBlur"))), } }
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "files" => <Vec<String> as Check>::describe(rest),
                "fontFamily" => <Vec<String> as Check>::describe(rest),
                "fontSize" => <Option<JsNumber> as Check>::describe(rest),
                "theme" => <Option<String> as Check>::describe(rest),
                "background" => <Option<TerminalRgb> as Check>::describe(rest),
                "foreground" => <Option<TerminalRgb> as Check>::describe(rest),
                "palette" => <Vec<Option<TerminalRgb>> as Check>::describe(rest),
                "selectionBackground" => <Option<TerminalRgb> as Check>::describe(rest),
                "cursorColor" => <Option<TerminalRgb> as Check>::describe(rest),
                "cursorStyle" => <Option<TerminalCursorStyle> as Check>::describe(rest),
                "cursorStyleBlink" => <Option<bool> as Check>::describe(rest),
                "windowPaddingX" => <Option<TerminalConfigWindowPaddingX> as Check>::describe(rest),
                "windowPaddingY" => <Option<TerminalConfigWindowPaddingY> as Check>::describe(rest),
                "backgroundOpacity" => <Option<JsNumber> as Check>::describe(rest),
                "backgroundBlur" => <Option<i64> as Check>::describe(rest),
                _ => None,
            },
        }
    }
}
impl TerminalConfig {
    fn rule_font_size(x: &Option<JsNumber>) -> Result<(), crate::types::check::SchemaError> {
        if let Some(v) = (*x) {
            crate::types::check::num_above(v.0, 0.0)?;
        }
        Ok(())
    }
    fn rule_palette(x: &Vec<Option<TerminalRgb>>) -> Result<(), crate::types::check::SchemaError> {
        crate::types::check::items_exactly((*x).len(), 16)?;
        Ok(())
    }
    fn rule_background_opacity(x: &Option<JsNumber>) -> Result<(), crate::types::check::SchemaError> {
        if let Some(v) = (*x) {
            crate::types::check::num_at_least(v.0, 0.0)?;
        }
        if let Some(v) = (*x) {
            crate::types::check::num_at_most(v.0, 1.0)?;
        }
        Ok(())
    }
    fn rule_background_blur(x: &Option<JsInt>) -> Result<(), crate::types::check::SchemaError> {
        if let Some(v) = (*x) {
            crate::types::check::num_at_least(v.0.0, 0.0)?;
        }
        Ok(())
    }
}
