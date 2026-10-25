// SPDX-License-Identifier: AGPL-3.0-only
//! Port of packages/protocol/src/views/project-bundle.ts at a3b870a6e: its zod shapes and plain constants, written by the generator.

use serde::{Deserialize, Serialize};

use crate::types::check::{Check, SchemaError};
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

impl Check for CredentialSignal {}

/// TS: packages/protocol/src/views/project-bundle.ts:16 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectRewrite {
    pub urls: Vec<String>,
    pub drop: Vec<String>,
}

impl Check for ProjectRewrite {
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}

/// TS: packages/protocol/src/views/project-bundle.ts:21 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectSecret {
    pub path: String,
    pub bytes: JsNumber,
    pub signals: Vec<CredentialSignal>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rewrite: Option<ProjectRewrite>,
}

impl Check for ProjectSecret {
    /// Nested checks: signals, rewrite.
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
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

impl Check for ProjectCarry {}

/// TS: packages/protocol/src/views/project-bundle.ts:31 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectAgent {
    pub agent: String,
    pub name: String,
    pub sessions: JsNumber,
    pub bytes: JsNumber,
    pub carry: ProjectCarry,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

impl Check for ProjectAgent {
    /// Nested checks: carry.
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
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
        todo!()
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub values: Option<Vec<String>>,
    pub excluded: Vec<String>,
    pub skipped: Vec<ProjectPlanSkippedItem>,
    pub agents: Vec<ProjectAgent>,
}

impl Check for ProjectPlan {
    /// Nested checks: secrets, skipped, agents.
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
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

impl Check for ProjectImportStage {}

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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bytes: Option<JsNumber>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub total: Option<JsNumber>,
}

impl ProjectImportEvent {
    /// The `type` key's value: the enum holding this struct writes and reads it as its tag.
    pub const TYPE: &'static str = "project.import";
}

impl Check for ProjectImportEvent {
    /// Nested checks: stage.
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
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

impl Check for ProjectAgentOutcome {}

/// TS: packages/protocol/src/views/project-bundle.ts:81 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectAgentResult {
    pub agent: String,
    pub files: JsNumber,
    pub bytes: JsNumber,
    pub outcome: ProjectAgentOutcome,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sessions: Option<JsNumber>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub skipped: Option<JsNumber>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rows: Option<JsNumber>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

impl Check for ProjectAgentResult {
    /// Nested checks: outcome.
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
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
        todo!()
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

impl Check for ProjectExportStage {}

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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bytes: Option<JsNumber>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub total: Option<JsNumber>,
}

impl ProjectExportEvent {
    /// The `type` key's value: the enum holding this struct writes and reads it as its tag.
    pub const TYPE: &'static str = "project.export";
}

impl Check for ProjectExportEvent {
    /// Nested checks: stage.
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
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
        todo!()
    }
}

/// TS: packages/protocol/src/views/project-bundle.ts:127 at a3b870a6e.
/// The daemon's own shape: wsp-frames declares it, with the same keys in the same order.
pub use wsp_frames::HostFolder;

impl Check for HostFolder {
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}

/// TS: packages/protocol/src/views/project-bundle.ts:140 at a3b870a6e.
/// The daemon's own shape: wsp-frames declares it, with the same keys in the same order.
pub use wsp_frames::HostFolderListing;

impl Check for HostFolderListing {
    /// Nested checks: folders.
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
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

impl Check for TerminalScheme {}

/// TS: packages/protocol/src/views/project-bundle.ts:154 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TerminalRgb {
    pub r: i64,
    pub g: i64,
    pub b: i64,
}

impl Check for TerminalRgb {
    /// Rules: r: >= 0; r: <= 255; g: >= 0; g: <= 255; b: >= 0; b: <= 255.
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
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

impl Check for TerminalCursorStyle {}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TerminalConfigWindowPaddingX {
    pub left: JsNumber,
    pub right: JsNumber,
}

impl Check for TerminalConfigWindowPaddingX {
    /// Rules: left: >= 0; right: >= 0.
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
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
        todo!()
    }
}

/// TS: packages/protocol/src/views/project-bundle.ts:164 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TerminalConfig {
    pub files: Vec<String>,
    pub font_family: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub font_size: Option<JsNumber>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub theme: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub background: Option<TerminalRgb>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub foreground: Option<TerminalRgb>,
    pub palette: Vec<Option<TerminalRgb>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub selection_background: Option<TerminalRgb>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cursor_color: Option<TerminalRgb>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cursor_style: Option<TerminalCursorStyle>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cursor_style_blink: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub window_padding_x: Option<TerminalConfigWindowPaddingX>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub window_padding_y: Option<TerminalConfigWindowPaddingY>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub background_opacity: Option<JsNumber>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub background_blur: Option<i64>,
}

impl Check for TerminalConfig {
    /// Rules: fontSize: > 0; palette: exactly 16 items; backgroundOpacity: >= 0; backgroundOpacity: <= 1; backgroundBlur: >= 0.
    /// Nested checks: background, foreground, palette, selectionBackground, cursorColor, cursorStyle, windowPaddingX, windowPaddingY.
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}
