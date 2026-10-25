// SPDX-License-Identifier: AGPL-3.0-only
//! Port of packages/protocol/src/views/workspace.ts at a3b870a6e: its zod shapes and plain constants, written by the generator.

use serde::{Deserialize, Serialize};

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
        crate::types::check::leaf(path, crate::types::check::Zod::Options(&["starting", "running", "paused", "gone"]))
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
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub expires_at: Option<JsNumber>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub offline: Option<bool>,
}

impl Check for ReachStatus {
    /// Nested checks: state.
    fn check(&self) -> Result<(), SchemaError> {
        self.state.check().map_err(|e| e.under("state"))?;
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        match obj.get("state") { None => out.push(crate::types::check::missing::<Self>("state")), Some(v) => { match crate::types::check::read::<ReachState>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("state"))), } } }
        if let Some(v) = obj.get("url") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("url"))), } }
        if let Some(v) = obj.get("expiresAt") { match crate::types::check::read_present::<JsNumber>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("expiresAt"))), } }
        if let Some(v) = obj.get("offline") { match crate::types::check::read_present::<bool>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("offline"))), } }
        Some(out)
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
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub offline: Option<bool>,
}

impl Check for ReachView {
    /// Nested checks: state.
    fn check(&self) -> Result<(), SchemaError> {
        self.state.check().map_err(|e| e.under("state"))?;
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        match obj.get("state") { None => out.push(crate::types::check::missing::<Self>("state")), Some(v) => { match crate::types::check::read::<ReachState>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("state"))), } } }
        if let Some(v) = obj.get("offline") { match crate::types::check::read_present::<bool>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("offline"))), } }
        Some(out)
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
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub daemon_token: Option<String>,
}

impl Check for DaemonReachView {
    fn check(&self) -> Result<(), SchemaError> {
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        match obj.get("url") { None => out.push(crate::types::check::missing::<Self>("url")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("url"))), } } }
        match obj.get("expiresAt") { None => out.push(crate::types::check::missing::<Self>("expiresAt")), Some(v) => { match crate::types::check::read::<JsNumber>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("expiresAt"))), } } }
        if let Some(v) = obj.get("daemonToken") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("daemonToken"))), } }
        Some(out)
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

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        match obj.get("url") { None => out.push(crate::types::check::missing::<Self>("url")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("url"))), } } }
        match obj.get("expiresAt") { None => out.push(crate::types::check::missing::<Self>("expiresAt")), Some(v) => { match crate::types::check::read::<JsNumber>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("expiresAt"))), } } }
        Some(out)
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
    pub status: JsInt,
    pub body: String,
}

impl Check for PortProbeView {
    fn check(&self) -> Result<(), SchemaError> {
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        match obj.get("status") { None => out.push(crate::types::check::missing::<Self>("status")), Some(v) => { match crate::types::check::read::<JsInt>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("status"))), } } }
        match obj.get("body") { None => out.push(crate::types::check::missing::<Self>("body")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("body"))), } } }
        Some(out)
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
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub size: Option<JsInt>,
}

impl Check for WorkspaceProject {
    /// Rules: size: >= 0.
    fn check(&self) -> Result<(), SchemaError> {
        if let Some(v) = self.size {
            crate::types::check::num_at_least(v.0.0, 0.0).map_err(|e| e.under("size"))?;
        }
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        match obj.get("name") { None => out.push(crate::types::check::missing::<Self>("name")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("name"))), } } }
        match obj.get("dest") { None => out.push(crate::types::check::missing::<Self>("dest")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("dest"))), } } }
        match obj.get("importedAt") { None => out.push(crate::types::check::missing::<Self>("importedAt")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("importedAt"))), } } }
        if let Some(v) = obj.get("size") { match crate::types::check::read_present::<JsInt>(v) { Ok(x) => { if let Err(e) = Self::rule_size(&x) { out.push(e.under("size")); } }, Err(es) => out.extend(es.into_iter().map(|e| e.under("size"))), } }
        Some(out)
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
impl WorkspaceProject {
    fn rule_size(x: &Option<JsInt>) -> Result<(), crate::types::check::SchemaError> {
        if let Some(v) = (*x) {
            crate::types::check::num_at_least(v.0.0, 0.0)?;
        }
        Ok(())
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

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        match obj.get("id") { None => out.push(crate::types::check::missing::<Self>("id")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("id"))), } } }
        match obj.get("name") { None => out.push(crate::types::check::missing::<Self>("name")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("name"))), } } }
        Some(out)
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
    pub bytes: JsInt,
    pub kind: SeedKind,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub row: Option<SeedFileRow>,
    pub ticked: bool,
}

impl Check for SeedFile {
    /// Rules: bytes: >= 0.
    /// Nested checks: kind, row.
    fn check(&self) -> Result<(), SchemaError> {
        crate::types::check::num_at_least(self.bytes.0.0, 0.0).map_err(|e| e.under("bytes"))?;
        self.kind.check().map_err(|e| e.under("kind"))?;
        self.row.check().map_err(|e| e.under("row"))?;
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        match obj.get("path") { None => out.push(crate::types::check::missing::<Self>("path")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("path"))), } } }
        match obj.get("dir") { None => out.push(crate::types::check::missing::<Self>("dir")), Some(v) => { match crate::types::check::read::<bool>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("dir"))), } } }
        match obj.get("bytes") { None => out.push(crate::types::check::missing::<Self>("bytes")), Some(v) => { match crate::types::check::read::<JsInt>(v) { Ok(x) => { if let Err(e) = Self::rule_bytes(&x) { out.push(e.under("bytes")); } }, Err(es) => out.extend(es.into_iter().map(|e| e.under("bytes"))), } } }
        match obj.get("kind") { None => out.push(crate::types::check::missing::<Self>("kind")), Some(v) => { match crate::types::check::read::<SeedKind>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("kind"))), } } }
        if let Some(v) = obj.get("row") { match crate::types::check::read_present::<SeedFileRow>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("row"))), } }
        match obj.get("ticked") { None => out.push(crate::types::check::missing::<Self>("ticked")), Some(v) => { match crate::types::check::read::<bool>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("ticked"))), } } }
        Some(out)
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
impl SeedFile {
    fn rule_bytes(x: &JsInt) -> Result<(), crate::types::check::SchemaError> {
        crate::types::check::num_at_least((*x).0.0, 0.0)?;
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SeedPlanUnpushed {
    pub commits: JsInt,
    pub base: String,
}

impl Check for SeedPlanUnpushed {
    fn check(&self) -> Result<(), SchemaError> {
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        match obj.get("commits") { None => out.push(crate::types::check::missing::<Self>("commits")), Some(v) => { match crate::types::check::read::<JsInt>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("commits"))), } } }
        match obj.get("base") { None => out.push(crate::types::check::missing::<Self>("base")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("base"))), } } }
        Some(out)
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
    pub files: JsInt,
    pub bytes: JsInt,
}

impl Check for SeedPlanMemory {
    fn check(&self) -> Result<(), SchemaError> {
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        match obj.get("key") { None => out.push(crate::types::check::missing::<Self>("key")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("key"))), } } }
        match obj.get("files") { None => out.push(crate::types::check::missing::<Self>("files")), Some(v) => { match crate::types::check::read::<JsInt>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("files"))), } } }
        match obj.get("bytes") { None => out.push(crate::types::check::missing::<Self>("bytes")), Some(v) => { match crate::types::check::read::<JsInt>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("bytes"))), } } }
        Some(out)
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
    pub uncommitted: JsInt,
    #[serde(deserialize_with = "crate::types::check::nullable")]
    pub memory: Option<SeedPlanMemory>,
    pub files: Vec<SeedFile>,
    pub remembered: bool,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub provider: Option<bool>,
}

impl Check for SeedPlan {
    /// Rules: uncommitted: >= 0.
    /// Nested checks: unpushed, memory, files.
    fn check(&self) -> Result<(), SchemaError> {
        self.unpushed.check().map_err(|e| e.under("unpushed"))?;
        crate::types::check::num_at_least(self.uncommitted.0.0, 0.0).map_err(|e| e.under("uncommitted"))?;
        self.memory.check().map_err(|e| e.under("memory"))?;
        self.files.check().map_err(|e| e.under("files"))?;
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        match obj.get("source") { None => out.push(crate::types::check::missing::<Self>("source")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("source"))), } } }
        match obj.get("remote") { None => out.push(crate::types::check::missing::<Self>("remote")), Some(v) => { match crate::types::check::read::<Option<String>>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("remote"))), } } }
        match obj.get("branch") { None => out.push(crate::types::check::missing::<Self>("branch")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("branch"))), } } }
        match obj.get("defaultBranch") { None => out.push(crate::types::check::missing::<Self>("defaultBranch")), Some(v) => { match crate::types::check::read::<Option<String>>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("defaultBranch"))), } } }
        match obj.get("unpushed") { None => out.push(crate::types::check::missing::<Self>("unpushed")), Some(v) => { match crate::types::check::read::<Option<SeedPlanUnpushed>>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("unpushed"))), } } }
        match obj.get("uncommitted") { None => out.push(crate::types::check::missing::<Self>("uncommitted")), Some(v) => { match crate::types::check::read::<JsInt>(v) { Ok(x) => { if let Err(e) = Self::rule_uncommitted(&x) { out.push(e.under("uncommitted")); } }, Err(es) => out.extend(es.into_iter().map(|e| e.under("uncommitted"))), } } }
        match obj.get("memory") { None => out.push(crate::types::check::missing::<Self>("memory")), Some(v) => { match crate::types::check::read::<Option<SeedPlanMemory>>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("memory"))), } } }
        match obj.get("files") { None => out.push(crate::types::check::missing::<Self>("files")), Some(v) => { match crate::types::check::read::<Vec<SeedFile>>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("files"))), } } }
        match obj.get("remembered") { None => out.push(crate::types::check::missing::<Self>("remembered")), Some(v) => { match crate::types::check::read::<bool>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("remembered"))), } } }
        if let Some(v) = obj.get("provider") { match crate::types::check::read_present::<bool>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("provider"))), } }
        Some(out)
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
impl SeedPlan {
    fn rule_uncommitted(x: &JsInt) -> Result<(), crate::types::check::SchemaError> {
        crate::types::check::num_at_least((*x).0.0, 0.0)?;
        Ok(())
    }
}

/// TS: packages/protocol/src/views/workspace.ts:139 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SeedChoice {
    pub files: Vec<String>,
    pub memory: bool,
    pub commits: bool,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub remember: Option<bool>,
}

impl Check for SeedChoice {
    fn check(&self) -> Result<(), SchemaError> {
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        match obj.get("files") { None => out.push(crate::types::check::missing::<Self>("files")), Some(v) => { match crate::types::check::read::<Vec<String>>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("files"))), } } }
        match obj.get("memory") { None => out.push(crate::types::check::missing::<Self>("memory")), Some(v) => { match crate::types::check::read::<bool>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("memory"))), } } }
        match obj.get("commits") { None => out.push(crate::types::check::missing::<Self>("commits")), Some(v) => { match crate::types::check::read::<bool>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("commits"))), } } }
        if let Some(v) = obj.get("remember") { match crate::types::check::read_present::<bool>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("remember"))), } }
        Some(out)
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
    pub bytes: JsInt,
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

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        match obj.get("path") { None => out.push(crate::types::check::missing::<Self>("path")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("path"))), } } }
        match obj.get("dir") { None => out.push(crate::types::check::missing::<Self>("dir")), Some(v) => { match crate::types::check::read::<bool>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("dir"))), } } }
        match obj.get("bytes") { None => out.push(crate::types::check::missing::<Self>("bytes")), Some(v) => { match crate::types::check::read::<JsInt>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("bytes"))), } } }
        match obj.get("kind") { None => out.push(crate::types::check::missing::<Self>("kind")), Some(v) => { match crate::types::check::read::<SeedKind>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("kind"))), } } }
        match obj.get("why") { None => out.push(crate::types::check::missing::<Self>("why")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("why"))), } } }
        match obj.get("picked") { None => out.push(crate::types::check::missing::<Self>("picked")), Some(v) => { match crate::types::check::read::<bool>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("picked"))), } } }
        Some(out)
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
    pub bytes: JsInt,
    pub missing: Vec<String>,
}

impl Check for ProjectSeedPicked {
    fn check(&self) -> Result<(), SchemaError> {
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        match obj.get("paths") { None => out.push(crate::types::check::missing::<Self>("paths")), Some(v) => { match crate::types::check::read::<Vec<String>>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("paths"))), } } }
        match obj.get("bytes") { None => out.push(crate::types::check::missing::<Self>("bytes")), Some(v) => { match crate::types::check::read::<JsInt>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("bytes"))), } } }
        match obj.get("missing") { None => out.push(crate::types::check::missing::<Self>("missing")), Some(v) => { match crate::types::check::read::<Vec<String>>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("missing"))), } } }
        Some(out)
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

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        match obj.get("project") { None => out.push(crate::types::check::missing::<Self>("project")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("project"))), } } }
        match obj.get("folder") { None => out.push(crate::types::check::missing::<Self>("folder")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("folder"))), } } }
        match obj.get("rows") { None => out.push(crate::types::check::missing::<Self>("rows")), Some(v) => { match crate::types::check::read::<Vec<SeedPickRow>>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("rows"))), } } }
        match obj.get("picked") { None => out.push(crate::types::check::missing::<Self>("picked")), Some(v) => { match crate::types::check::read::<ProjectSeedPicked>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("picked"))), } } }
        match obj.get("saved") { None => out.push(crate::types::check::missing::<Self>("saved")), Some(v) => { match crate::types::check::read::<bool>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("saved"))), } } }
        Some(out)
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

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        match obj.get("path") { None => out.push(crate::types::check::missing::<Self>("path")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("path"))), } } }
        Some(out)
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

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        match obj.get("url") { None => out.push(crate::types::check::missing::<Self>("url")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("url"))), } } }
        Some(out)
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

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        match obj.get("repo") { None => out.push(crate::types::check::missing::<Self>("repo")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("repo"))), } } }
        Some(out)
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

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        match obj.get("repo") { None => out.push(crate::types::check::missing::<Self>("repo")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("repo"))), } } }
        Some(out)
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

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let Some(tag) = obj.get("kind") else { return Some(vec![crate::types::check::missing::<Self>("kind")]); };
        match tag.as_str() {
            Some("folder") => { let mut body = obj.clone(); body.shift_remove("kind"); Some(crate::types::check::issues_of::<ProjectSourceFolder>(&serde_json::Value::Object(body))) }
            Some("git") => { let mut body = obj.clone(); body.shift_remove("kind"); Some(crate::types::check::issues_of::<ProjectSourceGit>(&serde_json::Value::Object(body))) }
            Some("github") => { let mut body = obj.clone(); body.shift_remove("kind"); Some(crate::types::check::issues_of::<ProjectSourceGithub>(&serde_json::Value::Object(body))) }
            Some("gitlab") => { let mut body = obj.clone(); body.shift_remove("kind"); Some(crate::types::check::issues_of::<ProjectSourceGitlab>(&serde_json::Value::Object(body))) }
            _ => Some(vec![crate::types::check::unknown_tag::<Self>("kind", tag)]),
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
    pub files: JsInt,
    pub bytes: JsInt,
    pub memory: ProjectViewSeededMemory,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub memory_files: Option<JsInt>,
    pub commits: JsInt,
    pub at: String,
}

impl Check for ProjectViewSeeded {
    fn check(&self) -> Result<(), SchemaError> {
        self.memory.check().map_err(|e| e.under("memory"))?;
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        match obj.get("files") { None => out.push(crate::types::check::missing::<Self>("files")), Some(v) => { match crate::types::check::read::<JsInt>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("files"))), } } }
        match obj.get("bytes") { None => out.push(crate::types::check::missing::<Self>("bytes")), Some(v) => { match crate::types::check::read::<JsInt>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("bytes"))), } } }
        match obj.get("memory") { None => out.push(crate::types::check::missing::<Self>("memory")), Some(v) => { match crate::types::check::read::<ProjectViewSeededMemory>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("memory"))), } } }
        if let Some(v) = obj.get("memoryFiles") { match crate::types::check::read_present::<JsInt>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("memoryFiles"))), } }
        match obj.get("commits") { None => out.push(crate::types::check::missing::<Self>("commits")), Some(v) => { match crate::types::check::read::<JsInt>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("commits"))), } } }
        match obj.get("at") { None => out.push(crate::types::check::missing::<Self>("at")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("at"))), } } }
        Some(out)
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

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        match obj.get("row") { None => out.push(crate::types::check::missing::<Self>("row")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("row"))), } } }
        match obj.get("command") { None => out.push(crate::types::check::missing::<Self>("command")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("command"))), } } }
        match obj.get("at") { None => out.push(crate::types::check::missing::<Self>("at")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("at"))), } } }
        match obj.get("seconds") { None => out.push(crate::types::check::missing::<Self>("seconds")), Some(v) => { match crate::types::check::read::<JsNumber>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("seconds"))), } } }
        Some(out)
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
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub lockfile_sha: Option<String>,
}

impl Check for ProjectViewImage {
    fn check(&self) -> Result<(), SchemaError> {
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        match obj.get("snapshotId") { None => out.push(crate::types::check::missing::<Self>("snapshotId")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("snapshotId"))), } } }
        match obj.get("builtAt") { None => out.push(crate::types::check::missing::<Self>("builtAt")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("builtAt"))), } } }
        if let Some(v) = obj.get("lockfileSha") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("lockfileSha"))), } }
        Some(out)
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

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        match obj.get("top") { None => out.push(crate::types::check::missing::<Self>("top")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("top"))), } } }
        Some(out)
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
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub checkout: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub seeded: Option<ProjectViewSeeded>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub installed: Option<ProjectViewInstalled>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub image: Option<ProjectViewImage>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub base: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
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

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        match obj.get("id") { None => out.push(crate::types::check::missing::<Self>("id")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("id"))), } } }
        match obj.get("name") { None => out.push(crate::types::check::missing::<Self>("name")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("name"))), } } }
        match obj.get("computer") { None => out.push(crate::types::check::missing::<Self>("computer")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("computer"))), } } }
        match obj.get("source") { None => out.push(crate::types::check::missing::<Self>("source")), Some(v) => { match crate::types::check::read::<ProjectSource>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("source"))), } } }
        match obj.get("path") { None => out.push(crate::types::check::missing::<Self>("path")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("path"))), } } }
        match obj.get("remote") { None => out.push(crate::types::check::missing::<Self>("remote")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("remote"))), } } }
        match obj.get("defaultBranch") { None => out.push(crate::types::check::missing::<Self>("defaultBranch")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("defaultBranch"))), } } }
        match obj.get("memoryKey") { None => out.push(crate::types::check::missing::<Self>("memoryKey")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("memoryKey"))), } } }
        match obj.get("memoryDir") { None => out.push(crate::types::check::missing::<Self>("memoryDir")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("memoryDir"))), } } }
        if let Some(v) = obj.get("checkout") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("checkout"))), } }
        if let Some(v) = obj.get("seeded") { match crate::types::check::read_present::<ProjectViewSeeded>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("seeded"))), } }
        if let Some(v) = obj.get("installed") { match crate::types::check::read_present::<ProjectViewInstalled>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("installed"))), } }
        if let Some(v) = obj.get("image") { match crate::types::check::read_present::<ProjectViewImage>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("image"))), } }
        if let Some(v) = obj.get("base") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("base"))), } }
        if let Some(v) = obj.get("git") { match crate::types::check::read_present::<ProjectViewGit>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("git"))), } }
        match obj.get("createdAt") { None => out.push(crate::types::check::missing::<Self>("createdAt")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("createdAt"))), } } }
        Some(out)
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
    pub repo_id: JsInt,
    pub owner: String,
    pub name: String,
    pub branch: String,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub start_sha: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub public: Option<True>,
}

impl Check for BranchOfRecord {
    /// Rules: repoId: > 0; owner: at least 1 UTF-16 units; name: at least 1 UTF-16 units; branch: at least 1 UTF-16 units; startSha: at least 1 UTF-16 units.
    fn check(&self) -> Result<(), SchemaError> {
        crate::types::check::num_above(self.repo_id.0.0, 0.0).map_err(|e| e.under("repoId"))?;
        crate::types::check::text_at_least(&self.owner, 1).map_err(|e| e.under("owner"))?;
        crate::types::check::text_at_least(&self.name, 1).map_err(|e| e.under("name"))?;
        crate::types::check::text_at_least(&self.branch, 1).map_err(|e| e.under("branch"))?;
        if let Some(v) = &self.start_sha {
            crate::types::check::text_at_least(v, 1).map_err(|e| e.under("startSha"))?;
        }
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        match obj.get("repoId") { None => out.push(crate::types::check::missing::<Self>("repoId")), Some(v) => { match crate::types::check::read::<JsInt>(v) { Ok(x) => { if let Err(e) = Self::rule_repo_id(&x) { out.push(e.under("repoId")); } }, Err(es) => out.extend(es.into_iter().map(|e| e.under("repoId"))), } } }
        match obj.get("owner") { None => out.push(crate::types::check::missing::<Self>("owner")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(x) => { if let Err(e) = Self::rule_owner(&x) { out.push(e.under("owner")); } }, Err(es) => out.extend(es.into_iter().map(|e| e.under("owner"))), } } }
        match obj.get("name") { None => out.push(crate::types::check::missing::<Self>("name")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(x) => { if let Err(e) = Self::rule_name(&x) { out.push(e.under("name")); } }, Err(es) => out.extend(es.into_iter().map(|e| e.under("name"))), } } }
        match obj.get("branch") { None => out.push(crate::types::check::missing::<Self>("branch")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(x) => { if let Err(e) = Self::rule_branch(&x) { out.push(e.under("branch")); } }, Err(es) => out.extend(es.into_iter().map(|e| e.under("branch"))), } } }
        if let Some(v) = obj.get("startSha") { match crate::types::check::read_present::<String>(v) { Ok(x) => { if let Err(e) = Self::rule_start_sha(&x) { out.push(e.under("startSha")); } }, Err(es) => out.extend(es.into_iter().map(|e| e.under("startSha"))), } }
        if let Some(v) = obj.get("public") { match crate::types::check::read_present::<True>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("public"))), } }
        Some(out)
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
impl BranchOfRecord {
    fn rule_repo_id(x: &JsInt) -> Result<(), crate::types::check::SchemaError> {
        crate::types::check::num_above((*x).0.0, 0.0)?;
        Ok(())
    }
    fn rule_owner(x: &String) -> Result<(), crate::types::check::SchemaError> {
        crate::types::check::text_at_least(x, 1)?;
        Ok(())
    }
    fn rule_name(x: &String) -> Result<(), crate::types::check::SchemaError> {
        crate::types::check::text_at_least(x, 1)?;
        Ok(())
    }
    fn rule_branch(x: &String) -> Result<(), crate::types::check::SchemaError> {
        crate::types::check::text_at_least(x, 1)?;
        Ok(())
    }
    fn rule_start_sha(x: &Option<String>) -> Result<(), crate::types::check::SchemaError> {
        if let Some(v) = x {
            crate::types::check::text_at_least(v, 1)?;
        }
        Ok(())
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
