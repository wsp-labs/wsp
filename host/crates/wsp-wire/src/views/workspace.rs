// SPDX-License-Identifier: AGPL-3.0-only
//! Port of packages/protocol/src/views/workspace.ts at a3b870a6e: its zod shapes and plain constants, written by the generator.

use serde::{Deserialize, Serialize};

use crate::literal;
use crate::types::changes::Checkout;
use crate::types::check::{Check, SchemaError};
use crate::types::js_number::JsNumber;
use crate::types::pull_request::PullRequestSeen;
use crate::types::start::ReviewDraft;
use crate::types::start::WorkspaceFrom;
use crate::types::tree::TreeFact;
use crate::types::workspace_look::WorkspaceGlyph;
use crate::types::workspace_look::WorkspaceTheme;
use crate::views::workspace_kind::WorkspaceKind;
use wsp_frames::PauseMode;
use wsp_frames::True;
use wsp_frames::WorkspaceSize;

literal!(ThreadLit, "thread");

/// TS: packages/protocol/src/views/workspace.ts:27 at a3b870a6e.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum WorkspacePhase {
    #[serde(rename = "running")]
    Running,
    #[serde(rename = "pausing")]
    Pausing,
    #[serde(rename = "napping")]
    Napping,
    #[serde(rename = "waking")]
    Waking,
    #[serde(rename = "gone")]
    Gone,
    #[serde(rename = "queued")]
    Queued,
    #[serde(rename = "released")]
    Released,
}

impl Check for WorkspacePhase {
    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        crate::types::check::leaf(path, crate::types::check::Zod::Options(&["running", "pausing", "napping", "waking", "gone", "queued", "released"]))
    }
}

/// TS: packages/protocol/src/views/workspace.ts:33 at a3b870a6e.
/// The daemon's own enum: wsp-frames declares it, with the same values in the same order.
pub use wsp_frames::MachineState;

impl Check for MachineState {
    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        crate::types::check::leaf(path, crate::types::check::Zod::Options(&["Starting", "Running", "Paused", "Gone"]))
    }
}

/// TS: packages/protocol/src/views/workspace.ts:43 at a3b870a6e.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ReachState {
    #[serde(rename = "reachable")]
    Reachable,
    #[serde(rename = "no-daemon")]
    NoDaemon,
    #[serde(rename = "unreachable")]
    Unreachable,
    #[serde(rename = "napping")]
    Napping,
    #[serde(rename = "unsupported")]
    Unsupported,
    #[serde(rename = "gone")]
    Gone,
    #[serde(rename = "slow")]
    Slow,
    #[serde(rename = "zombie")]
    Zombie,
}

impl Check for ReachState {
    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        crate::types::check::leaf(path, crate::types::check::Zod::Options(&["reachable", "no-daemon", "unreachable", "napping", "unsupported", "gone", "slow", "zombie"]))
    }
}

/// TS: packages/protocol/src/views/workspace.ts:46 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReachStatus {
    pub state: ReachState,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expires_at: Option<JsNumber>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub offline: Option<bool>,
}

impl Check for ReachStatus {
    /// Nested checks: state.
    fn check(&self) -> Result<(), SchemaError> {
        self.state.check().map_err(|e| e.under("state"))?;
        Ok(())
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "state" => <ReachState as Check>::describe(rest),
                "url" => <Option<String> as Check>::describe(rest),
                "expiresAt" => <Option<JsNumber> as Check>::describe(rest),
                "offline" => <Option<bool> as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

/// TS: packages/protocol/src/views/workspace.ts:59 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReachView {
    pub state: ReachState,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub offline: Option<bool>,
}

impl Check for ReachView {
    /// Nested checks: state.
    fn check(&self) -> Result<(), SchemaError> {
        self.state.check().map_err(|e| e.under("state"))?;
        Ok(())
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "state" => <ReachState as Check>::describe(rest),
                "offline" => <Option<bool> as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

/// TS: packages/protocol/src/views/workspace.ts:66 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DaemonReachView {
    pub url: String,
    pub expires_at: JsNumber,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub daemon_token: Option<String>,
}

impl Check for DaemonReachView {
    fn check(&self) -> Result<(), SchemaError> {
        Ok(())
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "url" => <String as Check>::describe(rest),
                "expiresAt" => <JsNumber as Check>::describe(rest),
                "daemonToken" => <Option<String> as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

/// TS: packages/protocol/src/views/workspace.ts:75 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PortReachView {
    pub url: String,
    pub expires_at: JsNumber,
}

impl Check for PortReachView {
    fn check(&self) -> Result<(), SchemaError> {
        Ok(())
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "url" => <String as Check>::describe(rest),
                "expiresAt" => <JsNumber as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

/// TS: packages/protocol/src/views/workspace.ts:82 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PortProbeView {
    pub status: i64,
    pub body: String,
}

impl Check for PortProbeView {
    fn check(&self) -> Result<(), SchemaError> {
        Ok(())
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "status" => <i64 as Check>::describe(rest),
                "body" => <String as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

/// TS: packages/protocol/src/views/workspace.ts:88 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceProject {
    pub name: String,
    pub dest: String,
    pub imported_at: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub size: Option<i64>,
}

impl Check for WorkspaceProject {
    /// Rules: size: >= 0.
    fn check(&self) -> Result<(), SchemaError> {
        if let Some(v) = self.size {
            crate::types::check::num_at_least(v as f64, 0.0).map_err(|e| e.under("size"))?;
        }
        Ok(())
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "name" => <String as Check>::describe(rest),
                "dest" => <String as Check>::describe(rest),
                "importedAt" => <String as Check>::describe(rest),
                "size" => <Option<i64> as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

/// TS: packages/protocol/src/views/workspace.ts:96 at a3b870a6e.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SeedKind {
    #[serde(rename = "config")]
    Config,
    #[serde(rename = "secret")]
    Secret,
    #[serde(rename = "rebuilt")]
    Rebuilt,
    #[serde(rename = "data")]
    Data,
    #[serde(rename = "never")]
    Never,
    #[serde(rename = "unknown")]
    Unknown,
}

impl Check for SeedKind {
    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        crate::types::check::leaf(path, crate::types::check::Zod::Options(&["config", "secret", "rebuilt", "data", "never", "unknown"]))
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SeedFileRow {
    pub id: String,
    pub name: String,
}

impl Check for SeedFileRow {
    fn check(&self) -> Result<(), SchemaError> {
        Ok(())
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "id" => <String as Check>::describe(rest),
                "name" => <String as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

/// TS: packages/protocol/src/views/workspace.ts:102 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SeedFile {
    pub path: String,
    pub dir: bool,
    pub bytes: i64,
    pub kind: SeedKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub row: Option<SeedFileRow>,
    pub ticked: bool,
}

impl Check for SeedFile {
    /// Rules: bytes: >= 0.
    /// Nested checks: kind, row.
    fn check(&self) -> Result<(), SchemaError> {
        crate::types::check::num_at_least(self.bytes as f64, 0.0).map_err(|e| e.under("bytes"))?;
        self.kind.check().map_err(|e| e.under("kind"))?;
        self.row.check().map_err(|e| e.under("row"))?;
        Ok(())
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "path" => <String as Check>::describe(rest),
                "dir" => <bool as Check>::describe(rest),
                "bytes" => <i64 as Check>::describe(rest),
                "kind" => <SeedKind as Check>::describe(rest),
                "row" => <Option<SeedFileRow> as Check>::describe(rest),
                "ticked" => <bool as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SeedPlanUnpushed {
    pub commits: i64,
    pub base: String,
}

impl Check for SeedPlanUnpushed {
    fn check(&self) -> Result<(), SchemaError> {
        Ok(())
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "commits" => <i64 as Check>::describe(rest),
                "base" => <String as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SeedPlanMemory {
    pub key: String,
    pub files: i64,
    pub bytes: i64,
}

impl Check for SeedPlanMemory {
    fn check(&self) -> Result<(), SchemaError> {
        Ok(())
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "key" => <String as Check>::describe(rest),
                "files" => <i64 as Check>::describe(rest),
                "bytes" => <i64 as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

/// TS: packages/protocol/src/views/workspace.ts:115 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SeedPlan {
    pub source: String,
    #[serde(deserialize_with = "crate::types::check::nullable")]
    pub remote: Option<String>,
    pub branch: String,
    #[serde(deserialize_with = "crate::types::check::nullable")]
    pub default_branch: Option<String>,
    #[serde(deserialize_with = "crate::types::check::nullable")]
    pub unpushed: Option<SeedPlanUnpushed>,
    pub uncommitted: i64,
    #[serde(deserialize_with = "crate::types::check::nullable")]
    pub memory: Option<SeedPlanMemory>,
    pub files: Vec<SeedFile>,
    pub remembered: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider: Option<bool>,
}

impl Check for SeedPlan {
    /// Rules: uncommitted: >= 0.
    /// Nested checks: unpushed, memory, files.
    fn check(&self) -> Result<(), SchemaError> {
        self.unpushed.check().map_err(|e| e.under("unpushed"))?;
        crate::types::check::num_at_least(self.uncommitted as f64, 0.0).map_err(|e| e.under("uncommitted"))?;
        self.memory.check().map_err(|e| e.under("memory"))?;
        self.files.check().map_err(|e| e.under("files"))?;
        Ok(())
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "source" => <String as Check>::describe(rest),
                "remote" => <Option<String> as Check>::describe(rest),
                "branch" => <String as Check>::describe(rest),
                "defaultBranch" => <Option<String> as Check>::describe(rest),
                "unpushed" => <Option<SeedPlanUnpushed> as Check>::describe(rest),
                "uncommitted" => <i64 as Check>::describe(rest),
                "memory" => <Option<SeedPlanMemory> as Check>::describe(rest),
                "files" => <Vec<SeedFile> as Check>::describe(rest),
                "remembered" => <bool as Check>::describe(rest),
                "provider" => <Option<bool> as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

/// TS: packages/protocol/src/views/workspace.ts:139 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SeedChoice {
    pub files: Vec<String>,
    pub memory: bool,
    pub commits: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub remember: Option<bool>,
}

impl Check for SeedChoice {
    fn check(&self) -> Result<(), SchemaError> {
        Ok(())
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "files" => <Vec<String> as Check>::describe(rest),
                "memory" => <bool as Check>::describe(rest),
                "commits" => <bool as Check>::describe(rest),
                "remember" => <Option<bool> as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

/// TS: packages/protocol/src/views/workspace.ts:149 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SeedPickRow {
    pub path: String,
    pub dir: bool,
    pub bytes: i64,
    pub kind: SeedKind,
    pub why: String,
    pub picked: bool,
}

impl Check for SeedPickRow {
    /// Nested checks: kind.
    fn check(&self) -> Result<(), SchemaError> {
        self.kind.check().map_err(|e| e.under("kind"))?;
        Ok(())
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "path" => <String as Check>::describe(rest),
                "dir" => <bool as Check>::describe(rest),
                "bytes" => <i64 as Check>::describe(rest),
                "kind" => <SeedKind as Check>::describe(rest),
                "why" => <String as Check>::describe(rest),
                "picked" => <bool as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectSeedPicked {
    pub paths: Vec<String>,
    pub bytes: i64,
    pub missing: Vec<String>,
}

impl Check for ProjectSeedPicked {
    fn check(&self) -> Result<(), SchemaError> {
        Ok(())
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "paths" => <Vec<String> as Check>::describe(rest),
                "bytes" => <i64 as Check>::describe(rest),
                "missing" => <Vec<String> as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

/// TS: packages/protocol/src/views/workspace.ts:155 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectSeed {
    pub project: String,
    pub folder: String,
    pub rows: Vec<SeedPickRow>,
    pub picked: ProjectSeedPicked,
    pub saved: bool,
}

impl Check for ProjectSeed {
    /// Nested checks: rows, picked.
    fn check(&self) -> Result<(), SchemaError> {
        self.rows.check().map_err(|e| e.under("rows"))?;
        self.picked.check().map_err(|e| e.under("picked"))?;
        Ok(())
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "project" => <String as Check>::describe(rest),
                "folder" => <String as Check>::describe(rest),
                "rows" => <Vec<SeedPickRow> as Check>::describe(rest),
                "picked" => <ProjectSeedPicked as Check>::describe(rest),
                "saved" => <bool as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

/// TS: packages/protocol/src/views/workspace.ts:167 at a3b870a6e.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ProjectAddStage {
    #[serde(rename = "planned")]
    Planned,
    #[serde(rename = "cloning")]
    Cloning,
    #[serde(rename = "seeding")]
    Seeding,
    #[serde(rename = "installing")]
    Installing,
    #[serde(rename = "imaging")]
    Imaging,
    #[serde(rename = "done")]
    Done,
    #[serde(rename = "failed")]
    Failed,
}

impl Check for ProjectAddStage {
    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        crate::types::check::leaf(path, crate::types::check::Zod::Options(&["planned", "cloning", "seeding", "installing", "imaging", "done", "failed"]))
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectSourceFolder {
    pub path: String,
}

impl ProjectSourceFolder {
    /// The `kind` key's value: the enum holding this struct writes and reads it as its tag.
    pub const KIND: &'static str = "folder";
}

impl Check for ProjectSourceFolder {
    fn check(&self) -> Result<(), SchemaError> {
        Ok(())
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "path" => <String as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectSourceGit {
    pub url: String,
}

impl ProjectSourceGit {
    /// The `kind` key's value: the enum holding this struct writes and reads it as its tag.
    pub const KIND: &'static str = "git";
}

impl Check for ProjectSourceGit {
    fn check(&self) -> Result<(), SchemaError> {
        Ok(())
    }

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

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectSourceGithub {
    pub repo: String,
}

impl ProjectSourceGithub {
    /// The `kind` key's value: the enum holding this struct writes and reads it as its tag.
    pub const KIND: &'static str = "github";
}

impl Check for ProjectSourceGithub {
    fn check(&self) -> Result<(), SchemaError> {
        Ok(())
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "repo" => <String as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectSourceGitlab {
    pub repo: String,
}

impl ProjectSourceGitlab {
    /// The `kind` key's value: the enum holding this struct writes and reads it as its tag.
    pub const KIND: &'static str = "gitlab";
}

impl Check for ProjectSourceGitlab {
    fn check(&self) -> Result<(), SchemaError> {
        Ok(())
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "repo" => <String as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

/// TS: packages/protocol/src/views/workspace.ts:172 at a3b870a6e.
/// zod's discriminatedUnion on `kind`: each variant's struct leaves that key to this tag.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind")]
pub enum ProjectSource {
    #[serde(rename = "folder")]
    Folder(ProjectSourceFolder),
    #[serde(rename = "git")]
    Git(ProjectSourceGit),
    #[serde(rename = "github")]
    Github(ProjectSourceGithub),
    #[serde(rename = "gitlab")]
    Gitlab(ProjectSourceGitlab),
}

impl Check for ProjectSource {
    fn check(&self) -> Result<(), SchemaError> {
        match self {
            ProjectSource::Folder(v) => v.check(),
            ProjectSource::Git(v) => v.check(),
            ProjectSource::Github(v) => v.check(),
            ProjectSource::Gitlab(v) => v.check(),
        }
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        if path.is_empty() { return Some(crate::types::check::Zod::Word("object")); }
        if path.len() == 1 && path[0].as_str() == Some("kind") { return Some(crate::types::check::Zod::Tags("kind", &["folder", "git", "github", "gitlab"])); }
        <ProjectSourceFolder as Check>::describe(path).or_else(|| <ProjectSourceGit as Check>::describe(path)).or_else(|| <ProjectSourceGithub as Check>::describe(path)).or_else(|| <ProjectSourceGitlab as Check>::describe(path))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ProjectViewSeededMemory {
    #[serde(rename = "landed")]
    Landed,
    #[serde(rename = "kept")]
    Kept,
    #[serde(rename = "none")]
    None,
}

impl Check for ProjectViewSeededMemory {
    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        crate::types::check::leaf(path, crate::types::check::Zod::Options(&["landed", "kept", "none"]))
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectViewSeeded {
    pub files: i64,
    pub bytes: i64,
    pub memory: ProjectViewSeededMemory,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub memory_files: Option<i64>,
    pub commits: i64,
    pub at: String,
}

impl Check for ProjectViewSeeded {
    fn check(&self) -> Result<(), SchemaError> {
        self.memory.check().map_err(|e| e.under("memory"))?;
        Ok(())
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "files" => <i64 as Check>::describe(rest),
                "bytes" => <i64 as Check>::describe(rest),
                "memory" => <ProjectViewSeededMemory as Check>::describe(rest),
                "memoryFiles" => <Option<i64> as Check>::describe(rest),
                "commits" => <i64 as Check>::describe(rest),
                "at" => <String as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectViewInstalled {
    pub row: String,
    pub command: String,
    pub at: String,
    pub seconds: JsNumber,
}

impl Check for ProjectViewInstalled {
    fn check(&self) -> Result<(), SchemaError> {
        Ok(())
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "row" => <String as Check>::describe(rest),
                "command" => <String as Check>::describe(rest),
                "at" => <String as Check>::describe(rest),
                "seconds" => <JsNumber as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectViewImage {
    pub snapshot_id: String,
    pub built_at: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lockfile_sha: Option<String>,
}

impl Check for ProjectViewImage {
    fn check(&self) -> Result<(), SchemaError> {
        Ok(())
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "snapshotId" => <String as Check>::describe(rest),
                "builtAt" => <String as Check>::describe(rest),
                "lockfileSha" => <Option<String> as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectViewGit {
    pub top: String,
}

impl Check for ProjectViewGit {
    fn check(&self) -> Result<(), SchemaError> {
        Ok(())
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "top" => <String as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

/// TS: packages/protocol/src/views/workspace.ts:184 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectView {
    pub id: String,
    pub name: String,
    pub computer: String,
    pub source: ProjectSource,
    pub path: String,
    pub remote: String,
    pub default_branch: String,
    pub memory_key: String,
    pub memory_dir: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub checkout: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub seeded: Option<ProjectViewSeeded>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub installed: Option<ProjectViewInstalled>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub image: Option<ProjectViewImage>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub base: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub git: Option<ProjectViewGit>,
    pub created_at: String,
}

impl Check for ProjectView {
    /// Nested checks: source, seeded, installed, image, git.
    fn check(&self) -> Result<(), SchemaError> {
        self.source.check().map_err(|e| e.under("source"))?;
        self.seeded.check().map_err(|e| e.under("seeded"))?;
        self.installed.check().map_err(|e| e.under("installed"))?;
        self.image.check().map_err(|e| e.under("image"))?;
        self.git.check().map_err(|e| e.under("git"))?;
        Ok(())
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "id" => <String as Check>::describe(rest),
                "name" => <String as Check>::describe(rest),
                "computer" => <String as Check>::describe(rest),
                "source" => <ProjectSource as Check>::describe(rest),
                "path" => <String as Check>::describe(rest),
                "remote" => <String as Check>::describe(rest),
                "defaultBranch" => <String as Check>::describe(rest),
                "memoryKey" => <String as Check>::describe(rest),
                "memoryDir" => <String as Check>::describe(rest),
                "checkout" => <Option<String> as Check>::describe(rest),
                "seeded" => <Option<ProjectViewSeeded> as Check>::describe(rest),
                "installed" => <Option<ProjectViewInstalled> as Check>::describe(rest),
                "image" => <Option<ProjectViewImage> as Check>::describe(rest),
                "base" => <Option<String> as Check>::describe(rest),
                "git" => <Option<ProjectViewGit> as Check>::describe(rest),
                "createdAt" => <String as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

/// TS: packages/protocol/src/views/workspace.ts:244 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BranchOfRecord {
    pub repo_id: i64,
    pub owner: String,
    pub name: String,
    pub branch: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub start_sha: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub public: Option<True>,
}

impl Check for BranchOfRecord {
    /// Rules: repoId: > 0; owner: at least 1 UTF-16 units; name: at least 1 UTF-16 units; branch: at least 1 UTF-16 units; startSha: at least 1 UTF-16 units.
    fn check(&self) -> Result<(), SchemaError> {
        crate::types::check::num_above(self.repo_id as f64, 0.0).map_err(|e| e.under("repoId"))?;
        crate::types::check::text_at_least(&self.owner, 1).map_err(|e| e.under("owner"))?;
        crate::types::check::text_at_least(&self.name, 1).map_err(|e| e.under("name"))?;
        crate::types::check::text_at_least(&self.branch, 1).map_err(|e| e.under("branch"))?;
        if let Some(v) = &self.start_sha {
            crate::types::check::text_at_least(v, 1).map_err(|e| e.under("startSha"))?;
        }
        Ok(())
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "repoId" => <i64 as Check>::describe(rest),
                "owner" => <String as Check>::describe(rest),
                "name" => <String as Check>::describe(rest),
                "branch" => <String as Check>::describe(rest),
                "startSha" => <Option<String> as Check>::describe(rest),
                "public" => None,
                _ => None,
            },
        }
    }
}

pub use super::workspace_more::*;

// ---- Hand-written below: the functions of the unit.

// views/workspace.ts:294 at a3b870a6e
// export type Caller = WorkspaceOrigin | { origin: WorkspaceOrigin; by: ThreadScope };
/// Who asked: a road alone, or a road and the thread scope it asked from.
#[derive(Clone, Debug, PartialEq)]
pub enum Caller {
    Road(WorkspaceOrigin),
    Scoped { origin: WorkspaceOrigin, by: ThreadScope },
}

// views/workspace.ts:297 at a3b870a6e
// export const roadOf = (caller: Caller | undefined): WorkspaceOrigin | undefined => (typeof caller === "string" ? caller : caller?.origin);
pub fn road_of(caller: Option<&Caller>) -> Option<WorkspaceOrigin> {
    todo!()
}

// views/workspace.ts:300 at a3b870a6e
// export const scopeOf = (caller: Caller | undefined): ThreadScope | undefined => (typeof caller === "string" ? undefined : caller?.by);
pub fn scope_of(caller: Option<&Caller>) -> Option<ThreadScope> {
    todo!()
}

// views/workspace.ts:450 at a3b870a6e
// export function agentsFrom(held: WorkspaceAgents | undefined, patch: Partial<WorkspaceAgents>): WorkspaceAgents {
//   return { ...(held ?? AGENTS_ON), ...patch };
// }
/// The held setting (crate::types::place_state's AGENTS_ON where none is held) with each field the patch names over it.
pub fn agents_from(held: Option<&WorkspaceAgents>, patch: &WorkspaceAgentsPatch) -> WorkspaceAgents {
    todo!()
}

/// A patch of WorkspaceAgents: an absent field keeps the held one.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct WorkspaceAgentsPatch {
    pub spawn: Option<bool>,
    pub max_machines: Option<i64>,
    pub max_depth: Option<i64>,
}
