// SPDX-License-Identifier: AGPL-3.0-only
//! Port of packages/protocol/src/views/snapshot-lineage.ts at a3b870a6e: its zod shapes and plain constants, written by the generator.

use serde::{Deserialize, Serialize};

use crate::literal;
use crate::types::check::{Check, SchemaError};
use crate::types::js_number::JsInt;
use crate::types::js_number::JsNumber;
use crate::views::golden_image::GoldenVersion;
use crate::views::golden_image::SealedImage;
use crate::views::golden_image::SealedImageCopy;
use crate::views::workspace::WorkspaceProject;
use crate::views::workspace::WorkspaceView;

literal!(UntouchedLit, "untouched");

/// TS: packages/protocol/src/views/snapshot-lineage.ts:12 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SnapshotLineage {
    pub name: String,
    #[serde(deserialize_with = "crate::types::check::nullable")]
    pub head: Option<JsNumber>,
    pub versions: Vec<GoldenVersion>,
}

impl Check for SnapshotLineage {
    /// Nested checks: versions.
    fn check(&self) -> Result<(), SchemaError> {
        self.versions.check().map_err(|e| e.under("versions"))?;
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        match obj.get("name") { None => out.push(crate::types::check::missing::<Self>("name")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("name"))), } } }
        match obj.get("head") { None => out.push(crate::types::check::missing::<Self>("head")), Some(v) => { match crate::types::check::read::<Option<JsNumber>>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("head"))), } } }
        match obj.get("versions") { None => out.push(crate::types::check::missing::<Self>("versions")), Some(v) => { match crate::types::check::read::<Vec<GoldenVersion>>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("versions"))), } } }
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "name" => <String as Check>::describe(rest),
                "head" => <Option<JsNumber> as Check>::describe(rest),
                "versions" => <Vec<GoldenVersion> as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

/// TS: packages/protocol/src/views/snapshot-lineage.ts:22 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectGolden {
    pub snapshot_id: String,
    pub projects: Vec<WorkspaceProject>,
    pub golden: String,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub version: Option<JsInt>,
    pub workspace_id: String,
    pub workspace_name: String,
    pub created_at: String,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub place: Option<String>,
}

impl Check for ProjectGolden {
    /// Nested checks: projects.
    fn check(&self) -> Result<(), SchemaError> {
        self.projects.check().map_err(|e| e.under("projects"))?;
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        match obj.get("snapshotId") { None => out.push(crate::types::check::missing::<Self>("snapshotId")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("snapshotId"))), } } }
        match obj.get("projects") { None => out.push(crate::types::check::missing::<Self>("projects")), Some(v) => { match crate::types::check::read::<Vec<WorkspaceProject>>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("projects"))), } } }
        match obj.get("golden") { None => out.push(crate::types::check::missing::<Self>("golden")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("golden"))), } } }
        if let Some(v) = obj.get("version") { match crate::types::check::read_present::<JsInt>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("version"))), } }
        match obj.get("workspaceId") { None => out.push(crate::types::check::missing::<Self>("workspaceId")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("workspaceId"))), } } }
        match obj.get("workspaceName") { None => out.push(crate::types::check::missing::<Self>("workspaceName")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("workspaceName"))), } } }
        match obj.get("createdAt") { None => out.push(crate::types::check::missing::<Self>("createdAt")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("createdAt"))), } } }
        if let Some(v) = obj.get("place") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("place"))), } }
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "snapshotId" => <String as Check>::describe(rest),
                "projects" => <Vec<WorkspaceProject> as Check>::describe(rest),
                "golden" => <String as Check>::describe(rest),
                "version" => <Option<i64> as Check>::describe(rest),
                "workspaceId" => <String as Check>::describe(rest),
                "workspaceName" => <String as Check>::describe(rest),
                "createdAt" => <String as Check>::describe(rest),
                "place" => <Option<String> as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

/// TS: packages/protocol/src/views/snapshot-lineage.ts:39 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectGoldenRemoved {
    pub project_golden: ProjectGolden,
    pub already_gone: bool,
}

impl Check for ProjectGoldenRemoved {
    /// Nested checks: projectGolden.
    fn check(&self) -> Result<(), SchemaError> {
        self.project_golden.check().map_err(|e| e.under("projectGolden"))?;
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        match obj.get("projectGolden") { None => out.push(crate::types::check::missing::<Self>("projectGolden")), Some(v) => { match crate::types::check::read::<ProjectGolden>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("projectGolden"))), } } }
        match obj.get("alreadyGone") { None => out.push(crate::types::check::missing::<Self>("alreadyGone")), Some(v) => { match crate::types::check::read::<bool>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("alreadyGone"))), } } }
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "projectGolden" => <ProjectGolden as Check>::describe(rest),
                "alreadyGone" => <bool as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

/// TS: packages/protocol/src/views/snapshot-lineage.ts:43 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SealedProjectImage {
    pub snapshot_id: String,
    pub projects: Vec<WorkspaceProject>,
    pub golden: String,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub version: Option<JsInt>,
    pub workspace_id: String,
    pub workspace_name: String,
    pub created_at: String,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub place: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub size_bytes: Option<JsNumber>,
}

impl Check for SealedProjectImage {
    /// Nested checks: projects.
    fn check(&self) -> Result<(), SchemaError> {
        self.projects.check().map_err(|e| e.under("projects"))?;
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        match obj.get("snapshotId") { None => out.push(crate::types::check::missing::<Self>("snapshotId")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("snapshotId"))), } } }
        match obj.get("projects") { None => out.push(crate::types::check::missing::<Self>("projects")), Some(v) => { match crate::types::check::read::<Vec<WorkspaceProject>>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("projects"))), } } }
        match obj.get("golden") { None => out.push(crate::types::check::missing::<Self>("golden")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("golden"))), } } }
        if let Some(v) = obj.get("version") { match crate::types::check::read_present::<JsInt>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("version"))), } }
        match obj.get("workspaceId") { None => out.push(crate::types::check::missing::<Self>("workspaceId")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("workspaceId"))), } } }
        match obj.get("workspaceName") { None => out.push(crate::types::check::missing::<Self>("workspaceName")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("workspaceName"))), } } }
        match obj.get("createdAt") { None => out.push(crate::types::check::missing::<Self>("createdAt")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("createdAt"))), } } }
        if let Some(v) = obj.get("place") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("place"))), } }
        if let Some(v) = obj.get("sizeBytes") { match crate::types::check::read_present::<JsNumber>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("sizeBytes"))), } }
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "snapshotId" => <String as Check>::describe(rest),
                "projects" => <Vec<WorkspaceProject> as Check>::describe(rest),
                "golden" => <String as Check>::describe(rest),
                "version" => <Option<i64> as Check>::describe(rest),
                "workspaceId" => <String as Check>::describe(rest),
                "workspaceName" => <String as Check>::describe(rest),
                "createdAt" => <String as Check>::describe(rest),
                "place" => <Option<String> as Check>::describe(rest),
                "sizeBytes" => <Option<JsNumber> as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

/// TS: packages/protocol/src/views/snapshot-lineage.ts:48 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SealedImageView {
    #[serde(deserialize_with = "crate::types::check::nullable")]
    pub image: Option<SealedImage>,
    pub copies: Vec<SealedImageCopy>,
    pub projects: Vec<SealedProjectImage>,
}

impl Check for SealedImageView {
    /// Nested checks: image, copies, projects.
    fn check(&self) -> Result<(), SchemaError> {
        self.image.check().map_err(|e| e.under("image"))?;
        self.copies.check().map_err(|e| e.under("copies"))?;
        self.projects.check().map_err(|e| e.under("projects"))?;
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        match obj.get("image") { None => out.push(crate::types::check::missing::<Self>("image")), Some(v) => { match crate::types::check::read::<Option<SealedImage>>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("image"))), } } }
        match obj.get("copies") { None => out.push(crate::types::check::missing::<Self>("copies")), Some(v) => { match crate::types::check::read::<Vec<SealedImageCopy>>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("copies"))), } } }
        match obj.get("projects") { None => out.push(crate::types::check::missing::<Self>("projects")), Some(v) => { match crate::types::check::read::<Vec<SealedProjectImage>>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("projects"))), } } }
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "image" => <Option<SealedImage> as Check>::describe(rest),
                "copies" => <Vec<SealedImageCopy> as Check>::describe(rest),
                "projects" => <Vec<SealedProjectImage> as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

/// TS: packages/protocol/src/views/snapshot-lineage.ts:56 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SnapshotGroup {
    pub count: JsNumber,
    pub bytes: JsNumber,
}

impl Check for SnapshotGroup {
    fn check(&self) -> Result<(), SchemaError> {
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        match obj.get("count") { None => out.push(crate::types::check::missing::<Self>("count")), Some(v) => { match crate::types::check::read::<JsNumber>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("count"))), } } }
        match obj.get("bytes") { None => out.push(crate::types::check::missing::<Self>("bytes")), Some(v) => { match crate::types::check::read::<JsNumber>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("bytes"))), } } }
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "count" => <JsNumber as Check>::describe(rest),
                "bytes" => <JsNumber as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SnapshotStorageNamed {
    pub count: JsNumber,
    pub free: JsNumber,
}

impl Check for SnapshotStorageNamed {
    fn check(&self) -> Result<(), SchemaError> {
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        match obj.get("count") { None => out.push(crate::types::check::missing::<Self>("count")), Some(v) => { match crate::types::check::read::<JsNumber>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("count"))), } } }
        match obj.get("free") { None => out.push(crate::types::check::missing::<Self>("free")), Some(v) => { match crate::types::check::read::<JsNumber>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("free"))), } } }
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "count" => <JsNumber as Check>::describe(rest),
                "free" => <JsNumber as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

/// TS: packages/protocol/src/views/snapshot-lineage.ts:63 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SnapshotStorage {
    pub count: JsNumber,
    pub total_bytes: JsNumber,
    pub free_gb: JsNumber,
    pub usd_per_gb_month: JsNumber,
    pub billed_from: String,
    pub monthly_usd: JsNumber,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub named: Option<SnapshotStorageNamed>,
    pub kept: SnapshotGroup,
    pub orphans: SnapshotGroup,
    pub others: SnapshotGroup,
}

impl Check for SnapshotStorage {
    /// Nested checks: named, kept, orphans, others.
    fn check(&self) -> Result<(), SchemaError> {
        self.named.check().map_err(|e| e.under("named"))?;
        self.kept.check().map_err(|e| e.under("kept"))?;
        self.orphans.check().map_err(|e| e.under("orphans"))?;
        self.others.check().map_err(|e| e.under("others"))?;
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        match obj.get("count") { None => out.push(crate::types::check::missing::<Self>("count")), Some(v) => { match crate::types::check::read::<JsNumber>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("count"))), } } }
        match obj.get("totalBytes") { None => out.push(crate::types::check::missing::<Self>("totalBytes")), Some(v) => { match crate::types::check::read::<JsNumber>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("totalBytes"))), } } }
        match obj.get("freeGb") { None => out.push(crate::types::check::missing::<Self>("freeGb")), Some(v) => { match crate::types::check::read::<JsNumber>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("freeGb"))), } } }
        match obj.get("usdPerGbMonth") { None => out.push(crate::types::check::missing::<Self>("usdPerGbMonth")), Some(v) => { match crate::types::check::read::<JsNumber>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("usdPerGbMonth"))), } } }
        match obj.get("billedFrom") { None => out.push(crate::types::check::missing::<Self>("billedFrom")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("billedFrom"))), } } }
        match obj.get("monthlyUsd") { None => out.push(crate::types::check::missing::<Self>("monthlyUsd")), Some(v) => { match crate::types::check::read::<JsNumber>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("monthlyUsd"))), } } }
        if let Some(v) = obj.get("named") { match crate::types::check::read_present::<SnapshotStorageNamed>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("named"))), } }
        match obj.get("kept") { None => out.push(crate::types::check::missing::<Self>("kept")), Some(v) => { match crate::types::check::read::<SnapshotGroup>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("kept"))), } } }
        match obj.get("orphans") { None => out.push(crate::types::check::missing::<Self>("orphans")), Some(v) => { match crate::types::check::read::<SnapshotGroup>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("orphans"))), } } }
        match obj.get("others") { None => out.push(crate::types::check::missing::<Self>("others")), Some(v) => { match crate::types::check::read::<SnapshotGroup>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("others"))), } } }
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "count" => <JsNumber as Check>::describe(rest),
                "totalBytes" => <JsNumber as Check>::describe(rest),
                "freeGb" => <JsNumber as Check>::describe(rest),
                "usdPerGbMonth" => <JsNumber as Check>::describe(rest),
                "billedFrom" => <String as Check>::describe(rest),
                "monthlyUsd" => <JsNumber as Check>::describe(rest),
                "named" => <Option<SnapshotStorageNamed> as Check>::describe(rest),
                "kept" => <SnapshotGroup as Check>::describe(rest),
                "orphans" => <SnapshotGroup as Check>::describe(rest),
                "others" => <SnapshotGroup as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

/// TS: packages/protocol/src/views/snapshot-lineage.ts:85 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SnapshotRollbackResult {
    pub lineage: SnapshotLineage,
    pub existing_workspaces: UntouchedLit,
}

impl Check for SnapshotRollbackResult {
    /// Nested checks: lineage.
    fn check(&self) -> Result<(), SchemaError> {
        self.lineage.check().map_err(|e| e.under("lineage"))?;
        self.existing_workspaces.check().map_err(|e| e.under("existingWorkspaces"))?;
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        match obj.get("lineage") { None => out.push(crate::types::check::missing::<Self>("lineage")), Some(v) => { match crate::types::check::read::<SnapshotLineage>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("lineage"))), } } }
        match obj.get("existingWorkspaces") { None => out.push(crate::types::check::missing::<Self>("existingWorkspaces")), Some(v) => { match crate::types::check::read::<UntouchedLit>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("existingWorkspaces"))), } } }
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "lineage" => <SnapshotLineage as Check>::describe(rest),
                "existingWorkspaces" => <UntouchedLit as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

/// TS: packages/protocol/src/views/snapshot-lineage.ts:93 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceCreateResult {
    pub workspace: WorkspaceView,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub notice: Option<String>,
}

impl Check for WorkspaceCreateResult {
    /// Nested checks: workspace.
    fn check(&self) -> Result<(), SchemaError> {
        self.workspace.check().map_err(|e| e.under("workspace"))?;
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        match obj.get("workspace") { None => out.push(crate::types::check::missing::<Self>("workspace")), Some(v) => { match crate::types::check::read::<WorkspaceView>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("workspace"))), } } }
        if let Some(v) = obj.get("notice") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("notice"))), } }
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "workspace" => <WorkspaceView as Check>::describe(rest),
                "notice" => <Option<String> as Check>::describe(rest),
                _ => None,
            },
        }
    }
}
