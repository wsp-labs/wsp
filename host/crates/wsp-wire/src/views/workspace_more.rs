// SPDX-License-Identifier: AGPL-3.0-only
//! The later shapes of packages/protocol/src/views/workspace.ts at a3b870a6e, apart only for the repo's file-size law; workspace.rs re-exports them.

#![allow(unused_imports)]

use serde::{Deserialize, Serialize};

use super::workspace::*;
use crate::literal;
use crate::types::changes::Checkout;
use crate::types::check::{Check, SchemaError};
use crate::types::js_number::JsInt;
use crate::types::js_number::JsNumber;
use crate::types::pull_request::PullRequestSeen;
use crate::types::start::ReviewDraft;
use crate::types::start::WorkspaceFrom;
use crate::types::tree::TreeFact;
use crate::types::workspace_look::WorkspaceGlyph;
use crate::types::workspace_look::WorkspaceTheme;
use crate::views::workspace_kind::WorkspaceKind;
use crate::wire::capabilities::PauseMode;
use wsp_frames::True;
use crate::wire::capabilities::WorkspaceSize;

/// TS: packages/protocol/src/views/workspace.ts:250 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectBranch {
    #[serde(deserialize_with = "crate::types::check::nullable")]
    pub branch: Option<String>,
    pub folder: bool,
}

impl Check for ProjectBranch {
    fn check(&self) -> Result<(), SchemaError> {
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        match obj.get("branch") { None => out.push(crate::types::check::missing::<Self>("branch")), Some(v) => { match crate::types::check::read::<Option<String>>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("branch"))), } } }
        match obj.get("folder") { None => out.push(crate::types::check::missing::<Self>("folder")), Some(v) => { match crate::types::check::read::<bool>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("folder"))), } } }
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "branch" => <Option<String> as Check>::describe(rest),
                "folder" => <bool as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

/// TS: packages/protocol/src/views/workspace.ts:255 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectRef {
    pub id: String,
    pub name: String,
    pub path: String,
    pub computer: String,
}

impl Check for ProjectRef {
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
        match obj.get("computer") { None => out.push(crate::types::check::missing::<Self>("computer")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("computer"))), } } }
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "id" => <String as Check>::describe(rest),
                "name" => <String as Check>::describe(rest),
                "path" => <String as Check>::describe(rest),
                "computer" => <String as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

/// TS: packages/protocol/src/views/workspace.ts:262 at a3b870a6e.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum DaemonKind {
    #[serde(rename = "cloud")]
    Cloud,
    #[serde(rename = "local")]
    Local,
    #[serde(rename = "place")]
    Place,
}

impl Check for DaemonKind {
    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        crate::types::check::leaf(path, crate::types::check::Zod::Options(&["cloud", "local", "place"]))
    }
}

/// TS: packages/protocol/src/views/workspace.ts:270 at a3b870a6e.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum WorkspaceOrigin {
    #[serde(rename = "here")]
    Here,
    #[serde(rename = "relayed")]
    Relayed,
    #[serde(rename = "paired")]
    Paired,
}

impl Check for WorkspaceOrigin {
    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        crate::types::check::leaf(path, crate::types::check::Zod::Options(&["here", "relayed", "paired"]))
    }
}

/// TS: packages/protocol/src/views/workspace.ts:276 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ThreadScope {
    pub kind: ThreadLit,
    pub thread_id: String,
    pub workspace_id: String,
    pub root_thread_id: String,
}

impl Check for ThreadScope {
    fn check(&self) -> Result<(), SchemaError> {
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        match obj.get("kind") { None => out.push(crate::types::check::missing::<Self>("kind")), Some(v) => { match crate::types::check::read::<ThreadLit>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("kind"))), } } }
        match obj.get("threadId") { None => out.push(crate::types::check::missing::<Self>("threadId")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("threadId"))), } } }
        match obj.get("workspaceId") { None => out.push(crate::types::check::missing::<Self>("workspaceId")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("workspaceId"))), } } }
        match obj.get("rootThreadId") { None => out.push(crate::types::check::missing::<Self>("rootThreadId")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("rootThreadId"))), } } }
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "kind" => <ThreadLit as Check>::describe(rest),
                "threadId" => <String as Check>::describe(rest),
                "workspaceId" => <String as Check>::describe(rest),
                "rootThreadId" => <String as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

/// TS: packages/protocol/src/views/workspace.ts:287 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EventAsker {
    pub thread_id: String,
    pub root_thread_id: String,
}

impl Check for EventAsker {
    fn check(&self) -> Result<(), SchemaError> {
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        match obj.get("threadId") { None => out.push(crate::types::check::missing::<Self>("threadId")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("threadId"))), } } }
        match obj.get("rootThreadId") { None => out.push(crate::types::check::missing::<Self>("rootThreadId")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("rootThreadId"))), } } }
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "threadId" => <String as Check>::describe(rest),
                "rootThreadId" => <String as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

/// TS: packages/protocol/src/views/workspace.ts:304 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceAgents {
    pub spawn: bool,
    pub max_machines: JsInt,
    pub max_depth: JsInt,
}

impl Check for WorkspaceAgents {
    /// Rules: maxMachines: >= 0; maxDepth: >= 1.
    fn check(&self) -> Result<(), SchemaError> {
        crate::types::check::num_at_least(self.max_machines.as_f64(), 0.0).map_err(|e| e.under("maxMachines"))?;
        crate::types::check::num_at_least(self.max_depth.as_f64(), 1.0).map_err(|e| e.under("maxDepth"))?;
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        match obj.get("spawn") { None => out.push(crate::types::check::missing::<Self>("spawn")), Some(v) => { match crate::types::check::read::<bool>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("spawn"))), } } }
        match obj.get("maxMachines") { None => out.push(crate::types::check::missing::<Self>("maxMachines")), Some(v) => { match crate::types::check::read::<JsInt>(v) { Ok(x) => { if let Err(e) = Self::rule_max_machines(&x) { out.push(e.under("maxMachines")); } }, Err(es) => out.extend(es.into_iter().map(|e| e.under("maxMachines"))), } } }
        match obj.get("maxDepth") { None => out.push(crate::types::check::missing::<Self>("maxDepth")), Some(v) => { match crate::types::check::read::<JsInt>(v) { Ok(x) => { if let Err(e) = Self::rule_max_depth(&x) { out.push(e.under("maxDepth")); } }, Err(es) => out.extend(es.into_iter().map(|e| e.under("maxDepth"))), } } }
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "spawn" => <bool as Check>::describe(rest),
                "maxMachines" => <JsInt as Check>::describe(rest),
                "maxDepth" => <JsInt as Check>::describe(rest),
                _ => None,
            },
        }
    }
}
impl WorkspaceAgents {
    fn rule_max_machines(x: &JsInt) -> Result<(), crate::types::check::SchemaError> {
        crate::types::check::num_at_least(x.as_f64(), 0.0)?;
        Ok(())
    }
    fn rule_max_depth(x: &JsInt) -> Result<(), crate::types::check::SchemaError> {
        crate::types::check::num_at_least(x.as_f64(), 1.0)?;
        Ok(())
    }
}

/// TS: packages/protocol/src/views/workspace.ts:316 at a3b870a6e.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum CopyRoad {
    #[serde(rename = "clonefile")]
    Clonefile,
    #[serde(rename = "worktree")]
    Worktree,
}

impl Check for CopyRoad {
    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        crate::types::check::leaf(path, crate::types::check::Zod::Options(&["clonefile", "worktree"]))
    }
}

/// TS: packages/protocol/src/views/workspace.ts:321 at a3b870a6e.
/// The daemon's own enum: wsp-frames declares it, with the same values in the same order.
pub use wsp_frames::Carried;

impl Check for Carried {
    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        crate::types::check::leaf(path, crate::types::check::Zod::Options(&["deps-and-config", "config-only", "nothing"]))
    }
}

/// TS: packages/protocol/src/views/workspace.ts:327 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CopyAsk {
    pub from: String,
    pub to: String,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub base: Option<String>,
    pub exclude: Vec<String>,
    pub size_line_bytes: JsInt,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub road: Option<CopyRoad>,
}

impl Check for CopyAsk {
    /// Rules: sizeLineBytes: >= 0.
    /// Nested checks: road.
    fn check(&self) -> Result<(), SchemaError> {
        crate::types::check::num_at_least(self.size_line_bytes.0.0, 0.0).map_err(|e| e.under("sizeLineBytes"))?;
        self.road.check().map_err(|e| e.under("road"))?;
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        match obj.get("from") { None => out.push(crate::types::check::missing::<Self>("from")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("from"))), } } }
        match obj.get("to") { None => out.push(crate::types::check::missing::<Self>("to")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("to"))), } } }
        if let Some(v) = obj.get("base") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("base"))), } }
        match obj.get("exclude") { None => out.push(crate::types::check::missing::<Self>("exclude")), Some(v) => { match crate::types::check::read::<Vec<String>>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("exclude"))), } } }
        match obj.get("sizeLineBytes") { None => out.push(crate::types::check::missing::<Self>("sizeLineBytes")), Some(v) => { match crate::types::check::read::<JsInt>(v) { Ok(x) => { if let Err(e) = Self::rule_size_line_bytes(&x) { out.push(e.under("sizeLineBytes")); } }, Err(es) => out.extend(es.into_iter().map(|e| e.under("sizeLineBytes"))), } } }
        if let Some(v) = obj.get("road") { match crate::types::check::read_present::<CopyRoad>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("road"))), } }
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "from" => <String as Check>::describe(rest),
                "to" => <String as Check>::describe(rest),
                "base" => <Option<String> as Check>::describe(rest),
                "exclude" => <Vec<String> as Check>::describe(rest),
                "sizeLineBytes" => <i64 as Check>::describe(rest),
                "road" => <Option<CopyRoad> as Check>::describe(rest),
                _ => None,
            },
        }
    }
}
impl CopyAsk {
    fn rule_size_line_bytes(x: &JsInt) -> Result<(), crate::types::check::SchemaError> {
        crate::types::check::num_at_least((*x).0.0, 0.0)?;
        Ok(())
    }
}

/// TS: packages/protocol/src/views/workspace.ts:343 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CopyReport {
    pub road: CopyRoad,
    pub path: String,
    pub base: String,
    pub branch: String,
    pub fetched: bool,
    pub carried: Carried,
    pub excluded: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub skipped: Option<Vec<String>>,
    pub bytes: JsInt,
    pub ms: JsInt,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub fell_back: Option<String>,
}

impl Check for CopyReport {
    /// Rules: bytes: >= 0; ms: >= 0.
    /// Nested checks: road, carried.
    fn check(&self) -> Result<(), SchemaError> {
        self.road.check().map_err(|e| e.under("road"))?;
        self.carried.check().map_err(|e| e.under("carried"))?;
        crate::types::check::num_at_least(self.bytes.0.0, 0.0).map_err(|e| e.under("bytes"))?;
        crate::types::check::num_at_least(self.ms.0.0, 0.0).map_err(|e| e.under("ms"))?;
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        match obj.get("road") { None => out.push(crate::types::check::missing::<Self>("road")), Some(v) => { match crate::types::check::read::<CopyRoad>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("road"))), } } }
        match obj.get("path") { None => out.push(crate::types::check::missing::<Self>("path")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("path"))), } } }
        match obj.get("base") { None => out.push(crate::types::check::missing::<Self>("base")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("base"))), } } }
        match obj.get("branch") { None => out.push(crate::types::check::missing::<Self>("branch")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("branch"))), } } }
        match obj.get("fetched") { None => out.push(crate::types::check::missing::<Self>("fetched")), Some(v) => { match crate::types::check::read::<bool>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("fetched"))), } } }
        match obj.get("carried") { None => out.push(crate::types::check::missing::<Self>("carried")), Some(v) => { match crate::types::check::read::<Carried>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("carried"))), } } }
        match obj.get("excluded") { None => out.push(crate::types::check::missing::<Self>("excluded")), Some(v) => { match crate::types::check::read::<Vec<String>>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("excluded"))), } } }
        if let Some(v) = obj.get("skipped") { match crate::types::check::read_present::<Vec<String>>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("skipped"))), } }
        match obj.get("bytes") { None => out.push(crate::types::check::missing::<Self>("bytes")), Some(v) => { match crate::types::check::read::<JsInt>(v) { Ok(x) => { if let Err(e) = Self::rule_bytes(&x) { out.push(e.under("bytes")); } }, Err(es) => out.extend(es.into_iter().map(|e| e.under("bytes"))), } } }
        match obj.get("ms") { None => out.push(crate::types::check::missing::<Self>("ms")), Some(v) => { match crate::types::check::read::<JsInt>(v) { Ok(x) => { if let Err(e) = Self::rule_ms(&x) { out.push(e.under("ms")); } }, Err(es) => out.extend(es.into_iter().map(|e| e.under("ms"))), } } }
        if let Some(v) = obj.get("fellBack") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("fellBack"))), } }
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "road" => <CopyRoad as Check>::describe(rest),
                "path" => <String as Check>::describe(rest),
                "base" => <String as Check>::describe(rest),
                "branch" => <String as Check>::describe(rest),
                "fetched" => <bool as Check>::describe(rest),
                "carried" => <Carried as Check>::describe(rest),
                "excluded" => <Vec<String> as Check>::describe(rest),
                "skipped" => <Option<Vec<String>> as Check>::describe(rest),
                "bytes" => <i64 as Check>::describe(rest),
                "ms" => <i64 as Check>::describe(rest),
                "fellBack" => <Option<String> as Check>::describe(rest),
                _ => None,
            },
        }
    }
}
impl CopyReport {
    fn rule_bytes(x: &JsInt) -> Result<(), crate::types::check::SchemaError> {
        crate::types::check::num_at_least((*x).0.0, 0.0)?;
        Ok(())
    }
    fn rule_ms(x: &JsInt) -> Result<(), crate::types::check::SchemaError> {
        crate::types::check::num_at_least((*x).0.0, 0.0)?;
        Ok(())
    }
}

/// TS: packages/protocol/src/views/workspace.ts:364 at a3b870a6e.
/// One module a worktree carries in beside the config files.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CarryModule {
    pub id: String,
    pub lockfiles: Vec<String>,
    pub carry: Vec<String>,
    pub never: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub installed: Option<String>,
}

impl Check for CarryModule {
    fn check(&self) -> Result<(), SchemaError> {
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        match obj.get("id") { None => out.push(crate::types::check::missing::<Self>("id")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("id"))), } } }
        match obj.get("lockfiles") { None => out.push(crate::types::check::missing::<Self>("lockfiles")), Some(v) => { match crate::types::check::read::<Vec<String>>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("lockfiles"))), } } }
        match obj.get("carry") { None => out.push(crate::types::check::missing::<Self>("carry")), Some(v) => { match crate::types::check::read::<Vec<String>>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("carry"))), } } }
        match obj.get("never") { None => out.push(crate::types::check::missing::<Self>("never")), Some(v) => { match crate::types::check::read::<Vec<String>>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("never"))), } } }
        if let Some(v) = obj.get("installed") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("installed"))), } }
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "id" => <String as Check>::describe(rest),
                "lockfiles" => <Vec<String> as Check>::describe(rest),
                "carry" => <Vec<String> as Check>::describe(rest),
                "never" => <Vec<String> as Check>::describe(rest),
                "installed" => <Option<String> as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

/// TS: packages/protocol/src/views/workspace.ts:371 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorktreeAsk {
    pub from: String,
    pub home: String,
    pub project: String,
    pub branch: String,
    pub modules: Vec<CarryModule>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub checkpoint: Option<String>,
}

impl Check for WorktreeAsk {
    /// Nested checks: modules.
    fn check(&self) -> Result<(), SchemaError> {
        self.modules.check().map_err(|e| e.under("modules"))?;
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        match obj.get("from") { None => out.push(crate::types::check::missing::<Self>("from")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("from"))), } } }
        match obj.get("home") { None => out.push(crate::types::check::missing::<Self>("home")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("home"))), } } }
        match obj.get("project") { None => out.push(crate::types::check::missing::<Self>("project")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("project"))), } } }
        match obj.get("branch") { None => out.push(crate::types::check::missing::<Self>("branch")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("branch"))), } } }
        match obj.get("modules") { None => out.push(crate::types::check::missing::<Self>("modules")), Some(v) => { match crate::types::check::read::<Vec<CarryModule>>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("modules"))), } } }
        if let Some(v) = obj.get("checkpoint") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("checkpoint"))), } }
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "from" => <String as Check>::describe(rest),
                "home" => <String as Check>::describe(rest),
                "project" => <String as Check>::describe(rest),
                "branch" => <String as Check>::describe(rest),
                "modules" => <Vec<CarryModule> as Check>::describe(rest),
                "checkpoint" => <Option<String> as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

/// TS: packages/protocol/src/views/workspace.ts:385 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectCopy {
    pub path: String,
    pub base: String,
    pub branch: String,
    pub carried: Carried,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub fell_back: Option<String>,
    pub road: CopyRoad,
    pub source: String,
}

impl Check for ProjectCopy {
    /// Nested checks: carried, road.
    fn check(&self) -> Result<(), SchemaError> {
        self.carried.check().map_err(|e| e.under("carried"))?;
        self.road.check().map_err(|e| e.under("road"))?;
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        match obj.get("path") { None => out.push(crate::types::check::missing::<Self>("path")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("path"))), } } }
        match obj.get("base") { None => out.push(crate::types::check::missing::<Self>("base")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("base"))), } } }
        match obj.get("branch") { None => out.push(crate::types::check::missing::<Self>("branch")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("branch"))), } } }
        match obj.get("carried") { None => out.push(crate::types::check::missing::<Self>("carried")), Some(v) => { match crate::types::check::read::<Carried>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("carried"))), } } }
        if let Some(v) = obj.get("fellBack") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("fellBack"))), } }
        match obj.get("road") { None => out.push(crate::types::check::missing::<Self>("road")), Some(v) => { match crate::types::check::read::<CopyRoad>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("road"))), } } }
        match obj.get("source") { None => out.push(crate::types::check::missing::<Self>("source")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("source"))), } } }
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "path" => <String as Check>::describe(rest),
                "base" => <String as Check>::describe(rest),
                "branch" => <String as Check>::describe(rest),
                "carried" => <Carried as Check>::describe(rest),
                "fellBack" => <Option<String> as Check>::describe(rest),
                "road" => <CopyRoad as Check>::describe(rest),
                "source" => <String as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum WorktreeSettledWhy {
    #[serde(rename = "merged")]
    Merged,
    #[serde(rename = "closed")]
    Closed,
    #[serde(rename = "deleted")]
    Deleted,
    #[serde(rename = "removed")]
    Removed,
}

impl Check for WorktreeSettledWhy {
    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        crate::types::check::leaf(path, crate::types::check::Zod::Options(&["merged", "closed", "deleted", "removed"]))
    }
}

/// TS: packages/protocol/src/views/workspace.ts:394 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorktreeSettled {
    pub at: JsNumber,
    pub why: WorktreeSettledWhy,
}

impl Check for WorktreeSettled {
    fn check(&self) -> Result<(), SchemaError> {
        self.why.check().map_err(|e| e.under("why"))?;
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        match obj.get("at") { None => out.push(crate::types::check::missing::<Self>("at")), Some(v) => { match crate::types::check::read::<JsNumber>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("at"))), } } }
        match obj.get("why") { None => out.push(crate::types::check::missing::<Self>("why")), Some(v) => { match crate::types::check::read::<WorktreeSettledWhy>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("why"))), } } }
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "at" => <JsNumber as Check>::describe(rest),
                "why" => <WorktreeSettledWhy as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

/// TS: packages/protocol/src/views/workspace.ts:402 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorktreeFolder {
    pub path: String,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub branch: Option<String>,
    pub made: bool,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub made_for: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub settled: Option<WorktreeSettled>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub kept: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub remove_failed: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub gone: Option<True>,
}

impl Check for WorktreeFolder {
    /// Nested checks: settled.
    fn check(&self) -> Result<(), SchemaError> {
        self.settled.check().map_err(|e| e.under("settled"))?;
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        match obj.get("path") { None => out.push(crate::types::check::missing::<Self>("path")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("path"))), } } }
        if let Some(v) = obj.get("branch") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("branch"))), } }
        match obj.get("made") { None => out.push(crate::types::check::missing::<Self>("made")), Some(v) => { match crate::types::check::read::<bool>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("made"))), } } }
        if let Some(v) = obj.get("madeFor") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("madeFor"))), } }
        if let Some(v) = obj.get("settled") { match crate::types::check::read_present::<WorktreeSettled>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("settled"))), } }
        if let Some(v) = obj.get("kept") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("kept"))), } }
        if let Some(v) = obj.get("removeFailed") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("removeFailed"))), } }
        if let Some(v) = obj.get("gone") { match crate::types::check::read_present::<True>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("gone"))), } }
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "path" => <String as Check>::describe(rest),
                "branch" => <Option<String> as Check>::describe(rest),
                "made" => <bool as Check>::describe(rest),
                "madeFor" => <Option<String> as Check>::describe(rest),
                "settled" => <Option<WorktreeSettled> as Check>::describe(rest),
                "kept" => <Option<String> as Check>::describe(rest),
                "removeFailed" => <Option<String> as Check>::describe(rest),
                "gone" => <Option<True> as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

/// TS: packages/protocol/src/views/workspace.ts:416 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FoundModule {
    pub id: String,
    pub folder: String,
    pub rebuild: bool,
}

impl Check for FoundModule {
    fn check(&self) -> Result<(), SchemaError> {
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        match obj.get("id") { None => out.push(crate::types::check::missing::<Self>("id")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("id"))), } } }
        match obj.get("folder") { None => out.push(crate::types::check::missing::<Self>("folder")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("folder"))), } } }
        match obj.get("rebuild") { None => out.push(crate::types::check::missing::<Self>("rebuild")), Some(v) => { match crate::types::check::read::<bool>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("rebuild"))), } } }
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "id" => <String as Check>::describe(rest),
                "folder" => <String as Check>::describe(rest),
                "rebuild" => <bool as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

/// TS: packages/protocol/src/views/workspace.ts:424 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorktreeReport {
    pub path: String,
    pub branch: String,
    pub made: bool,
    pub carried: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub plain: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub overlaid: Option<Vec<String>>,
    pub fresh: bool,
    pub modules: Vec<FoundModule>,
    pub ms: JsInt,
}

impl Check for WorktreeReport {
    /// Rules: ms: >= 0.
    /// Nested checks: modules.
    fn check(&self) -> Result<(), SchemaError> {
        self.modules.check().map_err(|e| e.under("modules"))?;
        crate::types::check::num_at_least(self.ms.0.0, 0.0).map_err(|e| e.under("ms"))?;
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        match obj.get("path") { None => out.push(crate::types::check::missing::<Self>("path")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("path"))), } } }
        match obj.get("branch") { None => out.push(crate::types::check::missing::<Self>("branch")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("branch"))), } } }
        match obj.get("made") { None => out.push(crate::types::check::missing::<Self>("made")), Some(v) => { match crate::types::check::read::<bool>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("made"))), } } }
        match obj.get("carried") { None => out.push(crate::types::check::missing::<Self>("carried")), Some(v) => { match crate::types::check::read::<Vec<String>>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("carried"))), } } }
        if let Some(v) = obj.get("plain") { match crate::types::check::read_present::<Vec<String>>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("plain"))), } }
        if let Some(v) = obj.get("overlaid") { match crate::types::check::read_present::<Vec<String>>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("overlaid"))), } }
        match obj.get("fresh") { None => out.push(crate::types::check::missing::<Self>("fresh")), Some(v) => { match crate::types::check::read::<bool>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("fresh"))), } } }
        match obj.get("modules") { None => out.push(crate::types::check::missing::<Self>("modules")), Some(v) => { match crate::types::check::read::<Vec<FoundModule>>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("modules"))), } } }
        match obj.get("ms") { None => out.push(crate::types::check::missing::<Self>("ms")), Some(v) => { match crate::types::check::read::<JsInt>(v) { Ok(x) => { if let Err(e) = Self::rule_ms(&x) { out.push(e.under("ms")); } }, Err(es) => out.extend(es.into_iter().map(|e| e.under("ms"))), } } }
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "path" => <String as Check>::describe(rest),
                "branch" => <String as Check>::describe(rest),
                "made" => <bool as Check>::describe(rest),
                "carried" => <Vec<String> as Check>::describe(rest),
                "plain" => <Option<Vec<String>> as Check>::describe(rest),
                "overlaid" => <Option<Vec<String>> as Check>::describe(rest),
                "fresh" => <bool as Check>::describe(rest),
                "modules" => <Vec<FoundModule> as Check>::describe(rest),
                "ms" => <i64 as Check>::describe(rest),
                _ => None,
            },
        }
    }
}
impl WorktreeReport {
    fn rule_ms(x: &JsInt) -> Result<(), crate::types::check::SchemaError> {
        crate::types::check::num_at_least((*x).0.0, 0.0)?;
        Ok(())
    }
}

/// TS: packages/protocol/src/views/workspace.ts:439 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorktreeRemoval {
    pub path: String,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub rescued: Option<String>,
}

impl Check for WorktreeRemoval {
    fn check(&self) -> Result<(), SchemaError> {
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        match obj.get("path") { None => out.push(crate::types::check::missing::<Self>("path")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("path"))), } } }
        if let Some(v) = obj.get("rescued") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("rescued"))), } }
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "path" => <String as Check>::describe(rest),
                "rescued" => <Option<String> as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

/// TS: packages/protocol/src/views/workspace.ts:444 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorktreeMade {
    pub path: String,
    pub branch: String,
    pub made: bool,
}

impl Check for WorktreeMade {
    fn check(&self) -> Result<(), SchemaError> {
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        match obj.get("path") { None => out.push(crate::types::check::missing::<Self>("path")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("path"))), } } }
        match obj.get("branch") { None => out.push(crate::types::check::missing::<Self>("branch")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("branch"))), } } }
        match obj.get("made") { None => out.push(crate::types::check::missing::<Self>("made")), Some(v) => { match crate::types::check::read::<bool>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("made"))), } } }
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "path" => <String as Check>::describe(rest),
                "branch" => <String as Check>::describe(rest),
                "made" => <bool as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum WorkspaceViewQueueWhy {
    #[serde(rename = "slot")]
    Slot,
    #[serde(rename = "minute")]
    Minute,
}

impl Check for WorkspaceViewQueueWhy {
    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        crate::types::check::leaf(path, crate::types::check::Zod::Options(&["slot", "minute"]))
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceViewQueue {
    pub since: String,
    pub why: WorkspaceViewQueueWhy,
}

impl Check for WorkspaceViewQueue {
    fn check(&self) -> Result<(), SchemaError> {
        self.why.check().map_err(|e| e.under("why"))?;
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        match obj.get("since") { None => out.push(crate::types::check::missing::<Self>("since")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("since"))), } } }
        match obj.get("why") { None => out.push(crate::types::check::missing::<Self>("why")), Some(v) => { match crate::types::check::read::<WorkspaceViewQueueWhy>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("why"))), } } }
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "since" => <String as Check>::describe(rest),
                "why" => <WorkspaceViewQueueWhy as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceViewScreen {
    pub stream_url: String,
}

impl Check for WorkspaceViewScreen {
    fn check(&self) -> Result<(), SchemaError> {
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        match obj.get("streamUrl") { None => out.push(crate::types::check::missing::<Self>("streamUrl")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("streamUrl"))), } } }
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "streamUrl" => <String as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceViewDaemonRefusedAt {
    pub machine_id: String,
    pub at: String,
    pub why: String,
}

impl Check for WorkspaceViewDaemonRefusedAt {
    fn check(&self) -> Result<(), SchemaError> {
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        match obj.get("machineId") { None => out.push(crate::types::check::missing::<Self>("machineId")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("machineId"))), } } }
        match obj.get("at") { None => out.push(crate::types::check::missing::<Self>("at")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("at"))), } } }
        match obj.get("why") { None => out.push(crate::types::check::missing::<Self>("why")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("why"))), } } }
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "machineId" => <String as Check>::describe(rest),
                "at" => <String as Check>::describe(rest),
                "why" => <String as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

/// TS: packages/protocol/src/views/workspace.ts:454 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceView {
    pub id: String,
    pub name: String,
    pub machine_id: String,
    pub phase: WorkspacePhase,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub kind: Option<WorkspaceKind>,
    pub golden: String,
    pub created_at: String,
    pub project: ProjectRef,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub folder: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub home: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub claude_session_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub queue: Option<WorkspaceViewQueue>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub screen: Option<WorkspaceViewScreen>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub gone: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub theme: Option<WorkspaceTheme>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub glyph: Option<WorkspaceGlyph>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub daemon_note: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub vaulted_at: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub vault_refused: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub daemon_refused_at: Option<WorkspaceViewDaemonRefusedAt>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub wake_refused: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub disk_gone: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub agents: Option<WorkspaceAgents>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub parent_thread_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub root_thread_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub parent_workspace_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub place: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub worktree: Option<WorktreeFolder>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub provider: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub pause_mode: Option<PauseMode>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub from: Option<WorkspaceFrom>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub review: Option<ReviewDraft>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub branch_of_record: Option<BranchOfRecord>,
}

impl Check for WorkspaceView {
    /// Nested checks: phase, kind, project, queue, screen, theme, glyph, daemonRefusedAt, agents, worktree, pauseMode, from, review, branchOfRecord.
    fn check(&self) -> Result<(), SchemaError> {
        self.kind.check().map_err(|e| e.under("kind"))?;
        self.project.check().map_err(|e| e.under("project"))?;
        self.queue.check().map_err(|e| e.under("queue"))?;
        self.screen.check().map_err(|e| e.under("screen"))?;
        self.theme.check().map_err(|e| e.under("theme"))?;
        self.glyph.check().map_err(|e| e.under("glyph"))?;
        self.daemon_refused_at.check().map_err(|e| e.under("daemonRefusedAt"))?;
        self.agents.check().map_err(|e| e.under("agents"))?;
        self.worktree.check().map_err(|e| e.under("worktree"))?;
        self.from.check().map_err(|e| e.under("from"))?;
        self.review.check().map_err(|e| e.under("review"))?;
        self.phase.check().map_err(|e| e.under("phase"))?;
        self.pause_mode.check().map_err(|e| e.under("pauseMode"))?;
        self.branch_of_record.check().map_err(|e| e.under("branchOfRecord"))?;
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        match obj.get("id") { None => out.push(crate::types::check::missing::<Self>("id")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("id"))), } } }
        match obj.get("name") { None => out.push(crate::types::check::missing::<Self>("name")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("name"))), } } }
        match obj.get("machineId") { None => out.push(crate::types::check::missing::<Self>("machineId")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("machineId"))), } } }
        match obj.get("phase") { None => out.push(crate::types::check::missing::<Self>("phase")), Some(v) => { match crate::types::check::read::<WorkspacePhase>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("phase"))), } } }
        if let Some(v) = obj.get("kind") { match crate::types::check::read_present::<WorkspaceKind>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("kind"))), } }
        match obj.get("golden") { None => out.push(crate::types::check::missing::<Self>("golden")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("golden"))), } } }
        match obj.get("createdAt") { None => out.push(crate::types::check::missing::<Self>("createdAt")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("createdAt"))), } } }
        match obj.get("project") { None => out.push(crate::types::check::missing::<Self>("project")), Some(v) => { match crate::types::check::read::<ProjectRef>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("project"))), } } }
        if let Some(v) = obj.get("folder") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("folder"))), } }
        if let Some(v) = obj.get("home") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("home"))), } }
        if let Some(v) = obj.get("claudeSessionId") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("claudeSessionId"))), } }
        if let Some(v) = obj.get("queue") { match crate::types::check::read_present::<WorkspaceViewQueue>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("queue"))), } }
        if let Some(v) = obj.get("screen") { match crate::types::check::read_present::<WorkspaceViewScreen>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("screen"))), } }
        if let Some(v) = obj.get("gone") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("gone"))), } }
        if let Some(v) = obj.get("theme") { match crate::types::check::read_present::<WorkspaceTheme>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("theme"))), } }
        if let Some(v) = obj.get("glyph") { match crate::types::check::read_present::<WorkspaceGlyph>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("glyph"))), } }
        if let Some(v) = obj.get("daemonNote") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("daemonNote"))), } }
        if let Some(v) = obj.get("vaultedAt") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("vaultedAt"))), } }
        if let Some(v) = obj.get("vaultRefused") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("vaultRefused"))), } }
        if let Some(v) = obj.get("daemonRefusedAt") { match crate::types::check::read_present::<WorkspaceViewDaemonRefusedAt>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("daemonRefusedAt"))), } }
        if let Some(v) = obj.get("wakeRefused") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("wakeRefused"))), } }
        if let Some(v) = obj.get("diskGone") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("diskGone"))), } }
        if let Some(v) = obj.get("agents") { match crate::types::check::read_present::<WorkspaceAgents>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("agents"))), } }
        if let Some(v) = obj.get("parentThreadId") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("parentThreadId"))), } }
        if let Some(v) = obj.get("rootThreadId") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("rootThreadId"))), } }
        if let Some(v) = obj.get("parentWorkspaceId") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("parentWorkspaceId"))), } }
        if let Some(v) = obj.get("place") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("place"))), } }
        if let Some(v) = obj.get("worktree") { match crate::types::check::read_present::<WorktreeFolder>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("worktree"))), } }
        if let Some(v) = obj.get("provider") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("provider"))), } }
        if let Some(v) = obj.get("pauseMode") { match crate::types::check::read_present::<PauseMode>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("pauseMode"))), } }
        if let Some(v) = obj.get("from") { match crate::types::check::read_present::<WorkspaceFrom>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("from"))), } }
        if let Some(v) = obj.get("review") { match crate::types::check::read_present::<ReviewDraft>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("review"))), } }
        if let Some(v) = obj.get("branchOfRecord") { match crate::types::check::read_present::<BranchOfRecord>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("branchOfRecord"))), } }
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "id" => <String as Check>::describe(rest),
                "name" => <String as Check>::describe(rest),
                "machineId" => <String as Check>::describe(rest),
                "phase" => None,
                "kind" => <Option<WorkspaceKind> as Check>::describe(rest),
                "golden" => <String as Check>::describe(rest),
                "createdAt" => <String as Check>::describe(rest),
                "project" => <ProjectRef as Check>::describe(rest),
                "folder" => <Option<String> as Check>::describe(rest),
                "home" => <Option<String> as Check>::describe(rest),
                "claudeSessionId" => <Option<String> as Check>::describe(rest),
                "queue" => <Option<WorkspaceViewQueue> as Check>::describe(rest),
                "screen" => <Option<WorkspaceViewScreen> as Check>::describe(rest),
                "gone" => <Option<String> as Check>::describe(rest),
                "theme" => <Option<WorkspaceTheme> as Check>::describe(rest),
                "glyph" => <Option<WorkspaceGlyph> as Check>::describe(rest),
                "daemonNote" => <Option<String> as Check>::describe(rest),
                "vaultedAt" => <Option<String> as Check>::describe(rest),
                "vaultRefused" => <Option<String> as Check>::describe(rest),
                "daemonRefusedAt" => <Option<WorkspaceViewDaemonRefusedAt> as Check>::describe(rest),
                "wakeRefused" => <Option<String> as Check>::describe(rest),
                "diskGone" => <Option<String> as Check>::describe(rest),
                "agents" => <Option<WorkspaceAgents> as Check>::describe(rest),
                "parentThreadId" => <Option<String> as Check>::describe(rest),
                "rootThreadId" => <Option<String> as Check>::describe(rest),
                "parentWorkspaceId" => <Option<String> as Check>::describe(rest),
                "place" => <Option<String> as Check>::describe(rest),
                "worktree" => <Option<WorktreeFolder> as Check>::describe(rest),
                "provider" => <Option<String> as Check>::describe(rest),
                "pauseMode" => None,
                "from" => <Option<WorkspaceFrom> as Check>::describe(rest),
                "review" => <Option<ReviewDraft> as Check>::describe(rest),
                "branchOfRecord" => None,
                _ => None,
            },
        }
    }
}

/// TS: packages/protocol/src/views/workspace.ts:553 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MachineFacts {
    pub os: String,
    pub uptime_ms: JsNumber,
    pub folder: String,
}

impl Check for MachineFacts {
    fn check(&self) -> Result<(), SchemaError> {
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        match obj.get("os") { None => out.push(crate::types::check::missing::<Self>("os")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("os"))), } } }
        match obj.get("uptimeMs") { None => out.push(crate::types::check::missing::<Self>("uptimeMs")), Some(v) => { match crate::types::check::read::<JsNumber>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("uptimeMs"))), } } }
        match obj.get("folder") { None => out.push(crate::types::check::missing::<Self>("folder")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("folder"))), } } }
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "os" => <String as Check>::describe(rest),
                "uptimeMs" => <JsNumber as Check>::describe(rest),
                "folder" => <String as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceStatusWakeAsk {
    pub ask: JsNumber,
    pub of: JsNumber,
}

impl Check for WorkspaceStatusWakeAsk {
    fn check(&self) -> Result<(), SchemaError> {
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        match obj.get("ask") { None => out.push(crate::types::check::missing::<Self>("ask")), Some(v) => { match crate::types::check::read::<JsNumber>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("ask"))), } } }
        match obj.get("of") { None => out.push(crate::types::check::missing::<Self>("of")), Some(v) => { match crate::types::check::read::<JsNumber>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("of"))), } } }
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "ask" => <JsNumber as Check>::describe(rest),
                "of" => <JsNumber as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

/// TS: packages/protocol/src/views/workspace.ts:557 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceStatus {
    pub id: String,
    pub name: String,
    pub machine_id: String,
    pub phase: WorkspacePhase,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub kind: Option<WorkspaceKind>,
    pub golden: String,
    pub created_at: String,
    pub project: ProjectRef,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub folder: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub home: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub claude_session_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub queue: Option<WorkspaceViewQueue>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub screen: Option<WorkspaceViewScreen>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub gone: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub theme: Option<WorkspaceTheme>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub glyph: Option<WorkspaceGlyph>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub daemon_note: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub vaulted_at: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub vault_refused: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub daemon_refused_at: Option<WorkspaceViewDaemonRefusedAt>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub wake_refused: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub disk_gone: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub agents: Option<WorkspaceAgents>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub parent_thread_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub root_thread_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub parent_workspace_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub place: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub worktree: Option<WorktreeFolder>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub provider: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub pause_mode: Option<PauseMode>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub from: Option<WorkspaceFrom>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub review: Option<ReviewDraft>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub branch_of_record: Option<BranchOfRecord>,
    pub machine_state: MachineState,
    pub reach: ReachStatus,
    pub size: WorkspaceSize,
    pub rate_usd_per_hour: JsNumber,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub facts: Option<MachineFacts>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub reason: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub wake_ask: Option<WorkspaceStatusWakeAsk>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub idle_at: Option<JsNumber>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub checkout: Option<Checkout>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub pr: Option<PullRequestSeen>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub tree: Option<TreeFact>,
}

impl Check for WorkspaceStatus {
    /// Nested checks: phase, kind, project, theme, glyph, agents, worktree, pauseMode, from, review, branchOfRecord, machineState, reach, size, facts, wakeAsk, checkout, pr, tree.
    fn check(&self) -> Result<(), SchemaError> {
        self.kind.check().map_err(|e| e.under("kind"))?;
        self.project.check().map_err(|e| e.under("project"))?;
        self.queue.check().map_err(|e| e.under("queue"))?;
        self.screen.check().map_err(|e| e.under("screen"))?;
        self.theme.check().map_err(|e| e.under("theme"))?;
        self.glyph.check().map_err(|e| e.under("glyph"))?;
        self.daemon_refused_at.check().map_err(|e| e.under("daemonRefusedAt"))?;
        self.agents.check().map_err(|e| e.under("agents"))?;
        self.worktree.check().map_err(|e| e.under("worktree"))?;
        self.from.check().map_err(|e| e.under("from"))?;
        self.review.check().map_err(|e| e.under("review"))?;
        self.facts.check().map_err(|e| e.under("facts"))?;
        self.wake_ask.check().map_err(|e| e.under("wakeAsk"))?;
        self.checkout.check().map_err(|e| e.under("checkout"))?;
        self.pr.check().map_err(|e| e.under("pr"))?;
        self.tree.check().map_err(|e| e.under("tree"))?;
        self.phase.check().map_err(|e| e.under("phase"))?;
        self.pause_mode.check().map_err(|e| e.under("pauseMode"))?;
        self.branch_of_record.check().map_err(|e| e.under("branchOfRecord"))?;
        self.machine_state.check().map_err(|e| e.under("machineState"))?;
        self.reach.check().map_err(|e| e.under("reach"))?;
        self.size.check().map_err(|e| e.under("size"))?;
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        match obj.get("id") { None => out.push(crate::types::check::missing::<Self>("id")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("id"))), } } }
        match obj.get("name") { None => out.push(crate::types::check::missing::<Self>("name")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("name"))), } } }
        match obj.get("machineId") { None => out.push(crate::types::check::missing::<Self>("machineId")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("machineId"))), } } }
        match obj.get("phase") { None => out.push(crate::types::check::missing::<Self>("phase")), Some(v) => { match crate::types::check::read::<WorkspacePhase>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("phase"))), } } }
        if let Some(v) = obj.get("kind") { match crate::types::check::read_present::<WorkspaceKind>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("kind"))), } }
        match obj.get("golden") { None => out.push(crate::types::check::missing::<Self>("golden")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("golden"))), } } }
        match obj.get("createdAt") { None => out.push(crate::types::check::missing::<Self>("createdAt")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("createdAt"))), } } }
        match obj.get("project") { None => out.push(crate::types::check::missing::<Self>("project")), Some(v) => { match crate::types::check::read::<ProjectRef>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("project"))), } } }
        if let Some(v) = obj.get("folder") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("folder"))), } }
        if let Some(v) = obj.get("home") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("home"))), } }
        if let Some(v) = obj.get("claudeSessionId") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("claudeSessionId"))), } }
        if let Some(v) = obj.get("queue") { match crate::types::check::read_present::<WorkspaceViewQueue>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("queue"))), } }
        if let Some(v) = obj.get("screen") { match crate::types::check::read_present::<WorkspaceViewScreen>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("screen"))), } }
        if let Some(v) = obj.get("gone") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("gone"))), } }
        if let Some(v) = obj.get("theme") { match crate::types::check::read_present::<WorkspaceTheme>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("theme"))), } }
        if let Some(v) = obj.get("glyph") { match crate::types::check::read_present::<WorkspaceGlyph>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("glyph"))), } }
        if let Some(v) = obj.get("daemonNote") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("daemonNote"))), } }
        if let Some(v) = obj.get("vaultedAt") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("vaultedAt"))), } }
        if let Some(v) = obj.get("vaultRefused") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("vaultRefused"))), } }
        if let Some(v) = obj.get("daemonRefusedAt") { match crate::types::check::read_present::<WorkspaceViewDaemonRefusedAt>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("daemonRefusedAt"))), } }
        if let Some(v) = obj.get("wakeRefused") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("wakeRefused"))), } }
        if let Some(v) = obj.get("diskGone") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("diskGone"))), } }
        if let Some(v) = obj.get("agents") { match crate::types::check::read_present::<WorkspaceAgents>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("agents"))), } }
        if let Some(v) = obj.get("parentThreadId") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("parentThreadId"))), } }
        if let Some(v) = obj.get("rootThreadId") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("rootThreadId"))), } }
        if let Some(v) = obj.get("parentWorkspaceId") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("parentWorkspaceId"))), } }
        if let Some(v) = obj.get("place") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("place"))), } }
        if let Some(v) = obj.get("worktree") { match crate::types::check::read_present::<WorktreeFolder>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("worktree"))), } }
        if let Some(v) = obj.get("provider") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("provider"))), } }
        if let Some(v) = obj.get("pauseMode") { match crate::types::check::read_present::<PauseMode>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("pauseMode"))), } }
        if let Some(v) = obj.get("from") { match crate::types::check::read_present::<WorkspaceFrom>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("from"))), } }
        if let Some(v) = obj.get("review") { match crate::types::check::read_present::<ReviewDraft>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("review"))), } }
        if let Some(v) = obj.get("branchOfRecord") { match crate::types::check::read_present::<BranchOfRecord>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("branchOfRecord"))), } }
        match obj.get("machineState") { None => out.push(crate::types::check::missing::<Self>("machineState")), Some(v) => { match crate::types::check::read::<MachineState>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("machineState"))), } } }
        match obj.get("reach") { None => out.push(crate::types::check::missing::<Self>("reach")), Some(v) => { match crate::types::check::read::<ReachStatus>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("reach"))), } } }
        match obj.get("size") { None => out.push(crate::types::check::missing::<Self>("size")), Some(v) => { match crate::types::check::read::<WorkspaceSize>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("size"))), } } }
        match obj.get("rateUsdPerHour") { None => out.push(crate::types::check::missing::<Self>("rateUsdPerHour")), Some(v) => { match crate::types::check::read::<JsNumber>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("rateUsdPerHour"))), } } }
        if let Some(v) = obj.get("facts") { match crate::types::check::read_present::<MachineFacts>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("facts"))), } }
        if let Some(v) = obj.get("reason") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("reason"))), } }
        if let Some(v) = obj.get("wakeAsk") { match crate::types::check::read_present::<WorkspaceStatusWakeAsk>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("wakeAsk"))), } }
        if let Some(v) = obj.get("idleAt") { match crate::types::check::read_present::<JsNumber>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("idleAt"))), } }
        if let Some(v) = obj.get("checkout") { match crate::types::check::read_present::<Checkout>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("checkout"))), } }
        if let Some(v) = obj.get("pr") { match crate::types::check::read_present::<PullRequestSeen>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("pr"))), } }
        if let Some(v) = obj.get("tree") { match crate::types::check::read_present::<TreeFact>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("tree"))), } }
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "id" => <String as Check>::describe(rest),
                "name" => <String as Check>::describe(rest),
                "machineId" => <String as Check>::describe(rest),
                "phase" => None,
                "kind" => <Option<WorkspaceKind> as Check>::describe(rest),
                "golden" => <String as Check>::describe(rest),
                "createdAt" => <String as Check>::describe(rest),
                "project" => <ProjectRef as Check>::describe(rest),
                "folder" => <Option<String> as Check>::describe(rest),
                "home" => <Option<String> as Check>::describe(rest),
                "claudeSessionId" => <Option<String> as Check>::describe(rest),
                "queue" => <Option<WorkspaceViewQueue> as Check>::describe(rest),
                "screen" => <Option<WorkspaceViewScreen> as Check>::describe(rest),
                "gone" => <Option<String> as Check>::describe(rest),
                "theme" => <Option<WorkspaceTheme> as Check>::describe(rest),
                "glyph" => <Option<WorkspaceGlyph> as Check>::describe(rest),
                "daemonNote" => <Option<String> as Check>::describe(rest),
                "vaultedAt" => <Option<String> as Check>::describe(rest),
                "vaultRefused" => <Option<String> as Check>::describe(rest),
                "daemonRefusedAt" => <Option<WorkspaceViewDaemonRefusedAt> as Check>::describe(rest),
                "wakeRefused" => <Option<String> as Check>::describe(rest),
                "diskGone" => <Option<String> as Check>::describe(rest),
                "agents" => <Option<WorkspaceAgents> as Check>::describe(rest),
                "parentThreadId" => <Option<String> as Check>::describe(rest),
                "rootThreadId" => <Option<String> as Check>::describe(rest),
                "parentWorkspaceId" => <Option<String> as Check>::describe(rest),
                "place" => <Option<String> as Check>::describe(rest),
                "worktree" => <Option<WorktreeFolder> as Check>::describe(rest),
                "provider" => <Option<String> as Check>::describe(rest),
                "pauseMode" => None,
                "from" => <Option<WorkspaceFrom> as Check>::describe(rest),
                "review" => <Option<ReviewDraft> as Check>::describe(rest),
                "branchOfRecord" => None,
                "machineState" => None,
                "reach" => None,
                "size" => None,
                "rateUsdPerHour" => <JsNumber as Check>::describe(rest),
                "facts" => <Option<MachineFacts> as Check>::describe(rest),
                "reason" => <Option<String> as Check>::describe(rest),
                "wakeAsk" => <Option<WorkspaceStatusWakeAsk> as Check>::describe(rest),
                "idleAt" => <Option<JsNumber> as Check>::describe(rest),
                "checkout" => <Option<Checkout> as Check>::describe(rest),
                "pr" => <Option<PullRequestSeen> as Check>::describe(rest),
                "tree" => <Option<TreeFact> as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

/// TS: packages/protocol/src/views/workspace.ts:596 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceOut {
    pub id: String,
    pub name: String,
    pub machine_id: String,
    pub phase: WorkspacePhase,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub kind: Option<WorkspaceKind>,
    pub golden: String,
    pub created_at: String,
    pub project: ProjectRef,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub folder: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub home: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub claude_session_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub gone: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub theme: Option<WorkspaceTheme>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub glyph: Option<WorkspaceGlyph>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub daemon_note: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub daemon_refused_at: Option<WorkspaceViewDaemonRefusedAt>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub vaulted_at: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub vault_refused: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub wake_refused: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub disk_gone: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub agents: Option<WorkspaceAgents>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub parent_thread_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub root_thread_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub parent_workspace_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub place: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub provider: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub pause_mode: Option<PauseMode>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub worktree: Option<WorktreeFolder>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub from: Option<WorkspaceFrom>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub review: Option<ReviewDraft>,
}

impl Check for WorkspaceOut {
    /// Nested checks: phase, kind, project, theme, glyph, agents, pauseMode, worktree, from, review.
    fn check(&self) -> Result<(), SchemaError> {
        self.kind.check().map_err(|e| e.under("kind"))?;
        self.project.check().map_err(|e| e.under("project"))?;
        self.theme.check().map_err(|e| e.under("theme"))?;
        self.glyph.check().map_err(|e| e.under("glyph"))?;
        self.daemon_refused_at.check().map_err(|e| e.under("daemonRefusedAt"))?;
        self.agents.check().map_err(|e| e.under("agents"))?;
        self.worktree.check().map_err(|e| e.under("worktree"))?;
        self.from.check().map_err(|e| e.under("from"))?;
        self.review.check().map_err(|e| e.under("review"))?;
        self.phase.check().map_err(|e| e.under("phase"))?;
        self.pause_mode.check().map_err(|e| e.under("pauseMode"))?;
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        match obj.get("id") { None => out.push(crate::types::check::missing::<Self>("id")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("id"))), } } }
        match obj.get("name") { None => out.push(crate::types::check::missing::<Self>("name")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("name"))), } } }
        match obj.get("machineId") { None => out.push(crate::types::check::missing::<Self>("machineId")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("machineId"))), } } }
        match obj.get("phase") { None => out.push(crate::types::check::missing::<Self>("phase")), Some(v) => { match crate::types::check::read::<WorkspacePhase>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("phase"))), } } }
        if let Some(v) = obj.get("kind") { match crate::types::check::read_present::<WorkspaceKind>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("kind"))), } }
        match obj.get("golden") { None => out.push(crate::types::check::missing::<Self>("golden")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("golden"))), } } }
        match obj.get("createdAt") { None => out.push(crate::types::check::missing::<Self>("createdAt")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("createdAt"))), } } }
        match obj.get("project") { None => out.push(crate::types::check::missing::<Self>("project")), Some(v) => { match crate::types::check::read::<ProjectRef>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("project"))), } } }
        if let Some(v) = obj.get("folder") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("folder"))), } }
        if let Some(v) = obj.get("home") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("home"))), } }
        if let Some(v) = obj.get("claudeSessionId") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("claudeSessionId"))), } }
        if let Some(v) = obj.get("gone") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("gone"))), } }
        if let Some(v) = obj.get("theme") { match crate::types::check::read_present::<WorkspaceTheme>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("theme"))), } }
        if let Some(v) = obj.get("glyph") { match crate::types::check::read_present::<WorkspaceGlyph>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("glyph"))), } }
        if let Some(v) = obj.get("daemonNote") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("daemonNote"))), } }
        if let Some(v) = obj.get("daemonRefusedAt") { match crate::types::check::read_present::<WorkspaceViewDaemonRefusedAt>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("daemonRefusedAt"))), } }
        if let Some(v) = obj.get("vaultedAt") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("vaultedAt"))), } }
        if let Some(v) = obj.get("vaultRefused") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("vaultRefused"))), } }
        if let Some(v) = obj.get("wakeRefused") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("wakeRefused"))), } }
        if let Some(v) = obj.get("diskGone") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("diskGone"))), } }
        if let Some(v) = obj.get("agents") { match crate::types::check::read_present::<WorkspaceAgents>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("agents"))), } }
        if let Some(v) = obj.get("parentThreadId") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("parentThreadId"))), } }
        if let Some(v) = obj.get("rootThreadId") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("rootThreadId"))), } }
        if let Some(v) = obj.get("parentWorkspaceId") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("parentWorkspaceId"))), } }
        if let Some(v) = obj.get("place") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("place"))), } }
        if let Some(v) = obj.get("provider") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("provider"))), } }
        if let Some(v) = obj.get("pauseMode") { match crate::types::check::read_present::<PauseMode>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("pauseMode"))), } }
        if let Some(v) = obj.get("worktree") { match crate::types::check::read_present::<WorktreeFolder>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("worktree"))), } }
        if let Some(v) = obj.get("from") { match crate::types::check::read_present::<WorkspaceFrom>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("from"))), } }
        if let Some(v) = obj.get("review") { match crate::types::check::read_present::<ReviewDraft>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("review"))), } }
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "id" => <String as Check>::describe(rest),
                "name" => <String as Check>::describe(rest),
                "machineId" => <String as Check>::describe(rest),
                "phase" => None,
                "kind" => <Option<WorkspaceKind> as Check>::describe(rest),
                "golden" => <String as Check>::describe(rest),
                "createdAt" => <String as Check>::describe(rest),
                "project" => <ProjectRef as Check>::describe(rest),
                "folder" => <Option<String> as Check>::describe(rest),
                "home" => <Option<String> as Check>::describe(rest),
                "claudeSessionId" => <Option<String> as Check>::describe(rest),
                "gone" => <Option<String> as Check>::describe(rest),
                "theme" => <Option<WorkspaceTheme> as Check>::describe(rest),
                "glyph" => <Option<WorkspaceGlyph> as Check>::describe(rest),
                "daemonNote" => <Option<String> as Check>::describe(rest),
                "daemonRefusedAt" => <Option<WorkspaceViewDaemonRefusedAt> as Check>::describe(rest),
                "vaultedAt" => <Option<String> as Check>::describe(rest),
                "vaultRefused" => <Option<String> as Check>::describe(rest),
                "wakeRefused" => <Option<String> as Check>::describe(rest),
                "diskGone" => <Option<String> as Check>::describe(rest),
                "agents" => <Option<WorkspaceAgents> as Check>::describe(rest),
                "parentThreadId" => <Option<String> as Check>::describe(rest),
                "rootThreadId" => <Option<String> as Check>::describe(rest),
                "parentWorkspaceId" => <Option<String> as Check>::describe(rest),
                "place" => <Option<String> as Check>::describe(rest),
                "provider" => <Option<String> as Check>::describe(rest),
                "pauseMode" => None,
                "worktree" => <Option<WorktreeFolder> as Check>::describe(rest),
                "from" => <Option<WorkspaceFrom> as Check>::describe(rest),
                "review" => <Option<ReviewDraft> as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

/// TS: packages/protocol/src/views/workspace.ts:602 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceTalked {
    pub id: String,
    pub name: String,
    pub machine_id: String,
    pub phase: WorkspacePhase,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub kind: Option<WorkspaceKind>,
    pub golden: String,
    pub created_at: String,
    pub project: ProjectRef,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub gone: Option<String>,
}

impl Check for WorkspaceTalked {
    /// Nested checks: phase, kind, project.
    fn check(&self) -> Result<(), SchemaError> {
        self.kind.check().map_err(|e| e.under("kind"))?;
        self.project.check().map_err(|e| e.under("project"))?;
        self.phase.check().map_err(|e| e.under("phase"))?;
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        match obj.get("id") { None => out.push(crate::types::check::missing::<Self>("id")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("id"))), } } }
        match obj.get("name") { None => out.push(crate::types::check::missing::<Self>("name")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("name"))), } } }
        match obj.get("machineId") { None => out.push(crate::types::check::missing::<Self>("machineId")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("machineId"))), } } }
        match obj.get("phase") { None => out.push(crate::types::check::missing::<Self>("phase")), Some(v) => { match crate::types::check::read::<WorkspacePhase>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("phase"))), } } }
        if let Some(v) = obj.get("kind") { match crate::types::check::read_present::<WorkspaceKind>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("kind"))), } }
        match obj.get("golden") { None => out.push(crate::types::check::missing::<Self>("golden")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("golden"))), } } }
        match obj.get("createdAt") { None => out.push(crate::types::check::missing::<Self>("createdAt")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("createdAt"))), } } }
        match obj.get("project") { None => out.push(crate::types::check::missing::<Self>("project")), Some(v) => { match crate::types::check::read::<ProjectRef>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("project"))), } } }
        if let Some(v) = obj.get("gone") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("gone"))), } }
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "id" => <String as Check>::describe(rest),
                "name" => <String as Check>::describe(rest),
                "machineId" => <String as Check>::describe(rest),
                "phase" => None,
                "kind" => <Option<WorkspaceKind> as Check>::describe(rest),
                "golden" => <String as Check>::describe(rest),
                "createdAt" => <String as Check>::describe(rest),
                "project" => <ProjectRef as Check>::describe(rest),
                "gone" => <Option<String> as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

/// TS: packages/protocol/src/views/workspace.ts:608 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceListing {
    pub id: String,
    pub name: String,
    pub machine_id: String,
    pub phase: WorkspacePhase,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub kind: Option<WorkspaceKind>,
    pub golden: String,
    pub created_at: String,
    pub project: ProjectRef,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub folder: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub home: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub claude_session_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub gone: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub theme: Option<WorkspaceTheme>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub glyph: Option<WorkspaceGlyph>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub daemon_note: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub daemon_refused_at: Option<WorkspaceViewDaemonRefusedAt>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub vaulted_at: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub vault_refused: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub wake_refused: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub disk_gone: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub agents: Option<WorkspaceAgents>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub parent_thread_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub root_thread_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub parent_workspace_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub place: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub provider: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub pause_mode: Option<PauseMode>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub worktree: Option<WorktreeFolder>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub from: Option<WorkspaceFrom>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub review: Option<ReviewDraft>,
    pub machine_state: MachineState,
    pub size: WorkspaceSize,
    pub rate_usd_per_hour: JsNumber,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub reason: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub idle_at: Option<JsNumber>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub facts: Option<MachineFacts>,
    pub reach: ReachView,
}

impl Check for WorkspaceListing {
    /// Nested checks: phase, kind, project, theme, glyph, agents, pauseMode, worktree, from, review, machineState, size, facts, reach.
    fn check(&self) -> Result<(), SchemaError> {
        self.kind.check().map_err(|e| e.under("kind"))?;
        self.project.check().map_err(|e| e.under("project"))?;
        self.theme.check().map_err(|e| e.under("theme"))?;
        self.glyph.check().map_err(|e| e.under("glyph"))?;
        self.daemon_refused_at.check().map_err(|e| e.under("daemonRefusedAt"))?;
        self.agents.check().map_err(|e| e.under("agents"))?;
        self.worktree.check().map_err(|e| e.under("worktree"))?;
        self.from.check().map_err(|e| e.under("from"))?;
        self.review.check().map_err(|e| e.under("review"))?;
        self.facts.check().map_err(|e| e.under("facts"))?;
        self.phase.check().map_err(|e| e.under("phase"))?;
        self.pause_mode.check().map_err(|e| e.under("pauseMode"))?;
        self.machine_state.check().map_err(|e| e.under("machineState"))?;
        self.size.check().map_err(|e| e.under("size"))?;
        self.reach.check().map_err(|e| e.under("reach"))?;
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        match obj.get("id") { None => out.push(crate::types::check::missing::<Self>("id")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("id"))), } } }
        match obj.get("name") { None => out.push(crate::types::check::missing::<Self>("name")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("name"))), } } }
        match obj.get("machineId") { None => out.push(crate::types::check::missing::<Self>("machineId")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("machineId"))), } } }
        match obj.get("phase") { None => out.push(crate::types::check::missing::<Self>("phase")), Some(v) => { match crate::types::check::read::<WorkspacePhase>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("phase"))), } } }
        if let Some(v) = obj.get("kind") { match crate::types::check::read_present::<WorkspaceKind>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("kind"))), } }
        match obj.get("golden") { None => out.push(crate::types::check::missing::<Self>("golden")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("golden"))), } } }
        match obj.get("createdAt") { None => out.push(crate::types::check::missing::<Self>("createdAt")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("createdAt"))), } } }
        match obj.get("project") { None => out.push(crate::types::check::missing::<Self>("project")), Some(v) => { match crate::types::check::read::<ProjectRef>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("project"))), } } }
        if let Some(v) = obj.get("folder") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("folder"))), } }
        if let Some(v) = obj.get("home") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("home"))), } }
        if let Some(v) = obj.get("claudeSessionId") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("claudeSessionId"))), } }
        if let Some(v) = obj.get("gone") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("gone"))), } }
        if let Some(v) = obj.get("theme") { match crate::types::check::read_present::<WorkspaceTheme>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("theme"))), } }
        if let Some(v) = obj.get("glyph") { match crate::types::check::read_present::<WorkspaceGlyph>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("glyph"))), } }
        if let Some(v) = obj.get("daemonNote") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("daemonNote"))), } }
        if let Some(v) = obj.get("daemonRefusedAt") { match crate::types::check::read_present::<WorkspaceViewDaemonRefusedAt>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("daemonRefusedAt"))), } }
        if let Some(v) = obj.get("vaultedAt") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("vaultedAt"))), } }
        if let Some(v) = obj.get("vaultRefused") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("vaultRefused"))), } }
        if let Some(v) = obj.get("wakeRefused") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("wakeRefused"))), } }
        if let Some(v) = obj.get("diskGone") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("diskGone"))), } }
        if let Some(v) = obj.get("agents") { match crate::types::check::read_present::<WorkspaceAgents>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("agents"))), } }
        if let Some(v) = obj.get("parentThreadId") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("parentThreadId"))), } }
        if let Some(v) = obj.get("rootThreadId") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("rootThreadId"))), } }
        if let Some(v) = obj.get("parentWorkspaceId") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("parentWorkspaceId"))), } }
        if let Some(v) = obj.get("place") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("place"))), } }
        if let Some(v) = obj.get("provider") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("provider"))), } }
        if let Some(v) = obj.get("pauseMode") { match crate::types::check::read_present::<PauseMode>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("pauseMode"))), } }
        if let Some(v) = obj.get("worktree") { match crate::types::check::read_present::<WorktreeFolder>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("worktree"))), } }
        if let Some(v) = obj.get("from") { match crate::types::check::read_present::<WorkspaceFrom>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("from"))), } }
        if let Some(v) = obj.get("review") { match crate::types::check::read_present::<ReviewDraft>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("review"))), } }
        match obj.get("machineState") { None => out.push(crate::types::check::missing::<Self>("machineState")), Some(v) => { match crate::types::check::read::<MachineState>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("machineState"))), } } }
        match obj.get("size") { None => out.push(crate::types::check::missing::<Self>("size")), Some(v) => { match crate::types::check::read::<WorkspaceSize>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("size"))), } } }
        match obj.get("rateUsdPerHour") { None => out.push(crate::types::check::missing::<Self>("rateUsdPerHour")), Some(v) => { match crate::types::check::read::<JsNumber>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("rateUsdPerHour"))), } } }
        if let Some(v) = obj.get("reason") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("reason"))), } }
        if let Some(v) = obj.get("idleAt") { match crate::types::check::read_present::<JsNumber>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("idleAt"))), } }
        if let Some(v) = obj.get("facts") { match crate::types::check::read_present::<MachineFacts>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("facts"))), } }
        match obj.get("reach") { None => out.push(crate::types::check::missing::<Self>("reach")), Some(v) => { match crate::types::check::read::<ReachView>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("reach"))), } } }
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "id" => <String as Check>::describe(rest),
                "name" => <String as Check>::describe(rest),
                "machineId" => <String as Check>::describe(rest),
                "phase" => None,
                "kind" => <Option<WorkspaceKind> as Check>::describe(rest),
                "golden" => <String as Check>::describe(rest),
                "createdAt" => <String as Check>::describe(rest),
                "project" => <ProjectRef as Check>::describe(rest),
                "folder" => <Option<String> as Check>::describe(rest),
                "home" => <Option<String> as Check>::describe(rest),
                "claudeSessionId" => <Option<String> as Check>::describe(rest),
                "gone" => <Option<String> as Check>::describe(rest),
                "theme" => <Option<WorkspaceTheme> as Check>::describe(rest),
                "glyph" => <Option<WorkspaceGlyph> as Check>::describe(rest),
                "daemonNote" => <Option<String> as Check>::describe(rest),
                "daemonRefusedAt" => <Option<WorkspaceViewDaemonRefusedAt> as Check>::describe(rest),
                "vaultedAt" => <Option<String> as Check>::describe(rest),
                "vaultRefused" => <Option<String> as Check>::describe(rest),
                "wakeRefused" => <Option<String> as Check>::describe(rest),
                "diskGone" => <Option<String> as Check>::describe(rest),
                "agents" => <Option<WorkspaceAgents> as Check>::describe(rest),
                "parentThreadId" => <Option<String> as Check>::describe(rest),
                "rootThreadId" => <Option<String> as Check>::describe(rest),
                "parentWorkspaceId" => <Option<String> as Check>::describe(rest),
                "place" => <Option<String> as Check>::describe(rest),
                "provider" => <Option<String> as Check>::describe(rest),
                "pauseMode" => None,
                "worktree" => <Option<WorktreeFolder> as Check>::describe(rest),
                "from" => <Option<WorkspaceFrom> as Check>::describe(rest),
                "review" => <Option<ReviewDraft> as Check>::describe(rest),
                "machineState" => None,
                "size" => None,
                "rateUsdPerHour" => <JsNumber as Check>::describe(rest),
                "reason" => <Option<String> as Check>::describe(rest),
                "idleAt" => <Option<JsNumber> as Check>::describe(rest),
                "facts" => <Option<MachineFacts> as Check>::describe(rest),
                "reach" => None,
                _ => None,
            },
        }
    }
}
