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
        crate::types::check::leaf(
            path,
            crate::types::check::Zod::Options(&["name", "mode", "keys", "pem", "url", "gitleaks", "catalog", "exports", "history"]),
        )
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
        crate::types::keys::req_key::<Vec<String>>(obj, "urls", &mut out);
        crate::types::keys::req_key::<Vec<String>>(obj, "drop", &mut out);
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
        crate::types::keys::req_key::<String>(obj, "path", &mut out);
        crate::types::keys::req_key::<JsNumber>(obj, "bytes", &mut out);
        crate::types::keys::req_key::<Vec<CredentialSignal>>(obj, "signals", &mut out);
        crate::types::keys::opt_key::<ProjectRewrite>(obj, "rewrite", &mut out);
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
        crate::types::keys::req_key::<String>(obj, "agent", &mut out);
        crate::types::keys::req_key::<String>(obj, "name", &mut out);
        crate::types::keys::req_key::<JsNumber>(obj, "sessions", &mut out);
        crate::types::keys::req_key::<JsNumber>(obj, "bytes", &mut out);
        crate::types::keys::req_key::<ProjectCarry>(obj, "carry", &mut out);
        crate::types::keys::opt_key::<String>(obj, "error", &mut out);
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
        crate::types::keys::req_key::<String>(obj, "path", &mut out);
        crate::types::keys::req_key::<String>(obj, "note", &mut out);
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
        crate::types::keys::req_key::<String>(obj, "source", &mut out);
        crate::types::keys::req_key::<bool>(obj, "repo", &mut out);
        crate::types::keys::req_key::<JsNumber>(obj, "files", &mut out);
        crate::types::keys::req_key::<JsNumber>(obj, "bytes", &mut out);
        crate::types::keys::req_key::<Vec<ProjectSecret>>(obj, "secrets", &mut out);
        crate::types::keys::opt_key::<Vec<String>>(obj, "values", &mut out);
        crate::types::keys::req_key::<Vec<String>>(obj, "excluded", &mut out);
        crate::types::keys::req_key::<Vec<ProjectPlanSkippedItem>>(obj, "skipped", &mut out);
        crate::types::keys::req_key::<Vec<ProjectAgent>>(obj, "agents", &mut out);
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
        crate::types::check::leaf(
            path,
            crate::types::check::Zod::Options(&["planned", "consented", "packing", "uploading", "landing", "done", "failed"]),
        )
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
        crate::types::keys::req_key::<String>(obj, "workspaceId", &mut out);
        crate::types::keys::req_key::<String>(obj, "source", &mut out);
        crate::types::keys::req_key::<String>(obj, "dest", &mut out);
        crate::types::keys::req_key::<ProjectImportStage>(obj, "stage", &mut out);
        crate::types::keys::req_key::<String>(obj, "message", &mut out);
        crate::types::keys::req_key::<JsNumber>(obj, "elapsedMs", &mut out);
        crate::types::keys::opt_key::<JsNumber>(obj, "bytes", &mut out);
        crate::types::keys::opt_key::<JsNumber>(obj, "total", &mut out);
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
        crate::types::keys::req_key::<String>(obj, "agent", &mut out);
        crate::types::keys::req_key::<JsNumber>(obj, "files", &mut out);
        crate::types::keys::req_key::<JsNumber>(obj, "bytes", &mut out);
        crate::types::keys::req_key::<ProjectAgentOutcome>(obj, "outcome", &mut out);
        crate::types::keys::opt_key::<JsNumber>(obj, "sessions", &mut out);
        crate::types::keys::opt_key::<JsNumber>(obj, "skipped", &mut out);
        crate::types::keys::opt_key::<String>(obj, "error", &mut out);
        crate::types::keys::opt_key::<JsNumber>(obj, "rows", &mut out);
        crate::types::keys::opt_key::<String>(obj, "note", &mut out);
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
        crate::types::keys::req_key::<String>(obj, "dest", &mut out);
        crate::types::keys::req_key::<JsNumber>(obj, "files", &mut out);
        crate::types::keys::req_key::<JsNumber>(obj, "bytes", &mut out);
        crate::types::keys::req_key::<JsNumber>(obj, "parts", &mut out);
        crate::types::keys::req_key::<Vec<String>>(obj, "cut", &mut out);
        crate::types::keys::req_key::<Vec<String>>(obj, "rewritten", &mut out);
        crate::types::keys::req_key::<Vec<ProjectAgentResult>>(obj, "agents", &mut out);
        crate::types::keys::req_key::<WorkspaceProject>(obj, "project", &mut out);
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
        crate::types::keys::req_key::<String>(obj, "workspaceId", &mut out);
        crate::types::keys::req_key::<String>(obj, "source", &mut out);
        crate::types::keys::req_key::<String>(obj, "dest", &mut out);
        crate::types::keys::req_key::<ProjectExportStage>(obj, "stage", &mut out);
        crate::types::keys::req_key::<String>(obj, "message", &mut out);
        crate::types::keys::req_key::<JsNumber>(obj, "elapsedMs", &mut out);
        crate::types::keys::opt_key::<JsNumber>(obj, "bytes", &mut out);
        crate::types::keys::opt_key::<JsNumber>(obj, "total", &mut out);
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
        crate::types::keys::req_key::<String>(obj, "dest", &mut out);
        crate::types::keys::req_key::<JsNumber>(obj, "files", &mut out);
        crate::types::keys::req_key::<JsNumber>(obj, "bytes", &mut out);
        crate::types::keys::req_key::<Vec<String>>(obj, "excluded", &mut out);
        crate::types::keys::req_key::<Vec<ProjectAgentResult>>(obj, "agents", &mut out);
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
        crate::types::keys::req_key::<String>(obj, "path", &mut out);
        crate::types::keys::req_key::<bool>(obj, "repo", &mut out);
        crate::types::keys::opt_key::<String>(obj, "branch", &mut out);
        crate::types::keys::opt_key::<JsNumber>(obj, "touchedAt", &mut out);
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
        crate::types::keys::req_key::<String>(obj, "dir", &mut out);
        crate::types::keys::req_key::<Vec<String>>(obj, "roots", &mut out);
        crate::types::keys::req_key::<Vec<HostFolder>>(obj, "folders", &mut out);
        crate::types::keys::req_key::<JsInt>(obj, "hidden", &mut out);
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
        crate::types::check::num_at_least(self.r.0 .0, 0.0).map_err(|e| e.under("r"))?;
        crate::types::check::num_at_most(self.r.0 .0, 255.0).map_err(|e| e.under("r"))?;
        crate::types::check::num_at_least(self.g.0 .0, 0.0).map_err(|e| e.under("g"))?;
        crate::types::check::num_at_most(self.g.0 .0, 255.0).map_err(|e| e.under("g"))?;
        crate::types::check::num_at_least(self.b.0 .0, 0.0).map_err(|e| e.under("b"))?;
        crate::types::check::num_at_most(self.b.0 .0, 255.0).map_err(|e| e.under("b"))?;
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        crate::types::keys::req_key_with::<JsInt>(obj, "r", &mut out, Self::rule_r);
        crate::types::keys::req_key_with::<JsInt>(obj, "g", &mut out, Self::rule_g);
        crate::types::keys::req_key_with::<JsInt>(obj, "b", &mut out, Self::rule_b);
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
        crate::types::check::num_at_least(x.0 .0, 0.0)?;
        crate::types::check::num_at_most(x.0 .0, 255.0)?;
        Ok(())
    }
    fn rule_g(x: &JsInt) -> Result<(), crate::types::check::SchemaError> {
        crate::types::check::num_at_least(x.0 .0, 0.0)?;
        crate::types::check::num_at_most(x.0 .0, 255.0)?;
        Ok(())
    }
    fn rule_b(x: &JsInt) -> Result<(), crate::types::check::SchemaError> {
        crate::types::check::num_at_least(x.0 .0, 0.0)?;
        crate::types::check::num_at_most(x.0 .0, 255.0)?;
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
        crate::types::keys::req_key_with::<JsNumber>(obj, "left", &mut out, Self::rule_left);
        crate::types::keys::req_key_with::<JsNumber>(obj, "right", &mut out, Self::rule_right);
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
        crate::types::check::num_at_least(x.0, 0.0)?;
        Ok(())
    }
    fn rule_right(x: &JsNumber) -> Result<(), crate::types::check::SchemaError> {
        crate::types::check::num_at_least(x.0, 0.0)?;
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
        crate::types::keys::req_key_with::<JsNumber>(obj, "top", &mut out, Self::rule_top);
        crate::types::keys::req_key_with::<JsNumber>(obj, "bottom", &mut out, Self::rule_bottom);
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
        crate::types::check::num_at_least(x.0, 0.0)?;
        Ok(())
    }
    fn rule_bottom(x: &JsNumber) -> Result<(), crate::types::check::SchemaError> {
        crate::types::check::num_at_least(x.0, 0.0)?;
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
            crate::types::check::num_at_least(v.0 .0, 0.0).map_err(|e| e.under("backgroundBlur"))?;
        }
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        crate::types::keys::req_key::<Vec<String>>(obj, "files", &mut out);
        crate::types::keys::req_key::<Vec<String>>(obj, "fontFamily", &mut out);
        crate::types::keys::opt_key_with::<JsNumber>(obj, "fontSize", &mut out, Self::rule_font_size);
        crate::types::keys::opt_key::<String>(obj, "theme", &mut out);
        crate::types::keys::opt_key::<TerminalRgb>(obj, "background", &mut out);
        crate::types::keys::opt_key::<TerminalRgb>(obj, "foreground", &mut out);
        crate::types::keys::req_key_with::<Vec<Option<TerminalRgb>>>(obj, "palette", &mut out, Self::rule_palette);
        crate::types::keys::opt_key::<TerminalRgb>(obj, "selectionBackground", &mut out);
        crate::types::keys::opt_key::<TerminalRgb>(obj, "cursorColor", &mut out);
        crate::types::keys::opt_key::<TerminalCursorStyle>(obj, "cursorStyle", &mut out);
        crate::types::keys::opt_key::<bool>(obj, "cursorStyleBlink", &mut out);
        crate::types::keys::opt_key::<TerminalConfigWindowPaddingX>(obj, "windowPaddingX", &mut out);
        crate::types::keys::opt_key::<TerminalConfigWindowPaddingY>(obj, "windowPaddingY", &mut out);
        crate::types::keys::opt_key_with::<JsNumber>(obj, "backgroundOpacity", &mut out, Self::rule_background_opacity);
        crate::types::keys::opt_key_with::<JsInt>(obj, "backgroundBlur", &mut out, Self::rule_background_blur);
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
        if let Some(v) = *x {
            crate::types::check::num_above(v.0, 0.0)?;
        }
        Ok(())
    }
    fn rule_palette(x: &Vec<Option<TerminalRgb>>) -> Result<(), crate::types::check::SchemaError> {
        crate::types::check::items_exactly((*x).len(), 16)?;
        Ok(())
    }
    fn rule_background_opacity(x: &Option<JsNumber>) -> Result<(), crate::types::check::SchemaError> {
        if let Some(v) = *x {
            crate::types::check::num_at_least(v.0, 0.0)?;
        }
        if let Some(v) = *x {
            crate::types::check::num_at_most(v.0, 1.0)?;
        }
        Ok(())
    }
    fn rule_background_blur(x: &Option<JsInt>) -> Result<(), crate::types::check::SchemaError> {
        if let Some(v) = *x {
            crate::types::check::num_at_least(v.0 .0, 0.0)?;
        }
        Ok(())
    }
}
