// SPDX-License-Identifier: AGPL-3.0-only
//! Port of packages/protocol/src/views/sub-row.ts at a3b870a6e: its zod shapes and plain constants, written by the generator.

use serde::{Deserialize, Serialize};

use crate::types::check::{Check, SchemaError};
use crate::types::js_number::JsInt;
use crate::types::js_number::JsNumber;
use wsp_frames::True;

/// TS: packages/protocol/src/views/sub-row.ts:55 at a3b870a6e.
pub const SUB_SPAWN_LOG_LINES: u64 = 40;

/// TS: packages/protocol/src/views/sub-row.ts:12 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SubResult {
    pub branch: String,
    #[serde(deserialize_with = "crate::types::check::nullable")]
    pub pushed_sha: Option<String>,
    pub commits: JsInt,
    pub unpushed: JsInt,
    pub changed: JsInt,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub on_branch: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub norepo: Option<True>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub read_failed: Option<String>,
    pub read_at: String,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub push_refused: Option<String>,
}

impl Check for SubResult {
    /// Rules: commits: >= 0; unpushed: >= 0; changed: >= 0.
    fn check(&self) -> Result<(), SchemaError> {
        crate::types::check::num_at_least(self.commits.0.0, 0.0).map_err(|e| e.under("commits"))?;
        crate::types::check::num_at_least(self.unpushed.0.0, 0.0).map_err(|e| e.under("unpushed"))?;
        crate::types::check::num_at_least(self.changed.0.0, 0.0).map_err(|e| e.under("changed"))?;
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        match obj.get("branch") { None => out.push(crate::types::check::missing::<Self>("branch")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("branch"))), } } }
        match obj.get("pushedSha") { None => out.push(crate::types::check::missing::<Self>("pushedSha")), Some(v) => { match crate::types::check::read::<Option<String>>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("pushedSha"))), } } }
        match obj.get("commits") { None => out.push(crate::types::check::missing::<Self>("commits")), Some(v) => { match crate::types::check::read::<JsInt>(v) { Ok(x) => { if let Err(e) = Self::rule_commits(&x) { out.push(e.under("commits")); } }, Err(es) => out.extend(es.into_iter().map(|e| e.under("commits"))), } } }
        match obj.get("unpushed") { None => out.push(crate::types::check::missing::<Self>("unpushed")), Some(v) => { match crate::types::check::read::<JsInt>(v) { Ok(x) => { if let Err(e) = Self::rule_unpushed(&x) { out.push(e.under("unpushed")); } }, Err(es) => out.extend(es.into_iter().map(|e| e.under("unpushed"))), } } }
        match obj.get("changed") { None => out.push(crate::types::check::missing::<Self>("changed")), Some(v) => { match crate::types::check::read::<JsInt>(v) { Ok(x) => { if let Err(e) = Self::rule_changed(&x) { out.push(e.under("changed")); } }, Err(es) => out.extend(es.into_iter().map(|e| e.under("changed"))), } } }
        if let Some(v) = obj.get("onBranch") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("onBranch"))), } }
        if let Some(v) = obj.get("norepo") { match crate::types::check::read_present::<True>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("norepo"))), } }
        if let Some(v) = obj.get("readFailed") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("readFailed"))), } }
        match obj.get("readAt") { None => out.push(crate::types::check::missing::<Self>("readAt")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("readAt"))), } } }
        if let Some(v) = obj.get("pushRefused") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("pushRefused"))), } }
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "branch" => <String as Check>::describe(rest),
                "pushedSha" => <Option<String> as Check>::describe(rest),
                "commits" => <i64 as Check>::describe(rest),
                "unpushed" => <i64 as Check>::describe(rest),
                "changed" => <i64 as Check>::describe(rest),
                "onBranch" => <Option<String> as Check>::describe(rest),
                "norepo" => None,
                "readFailed" => <Option<String> as Check>::describe(rest),
                "readAt" => <String as Check>::describe(rest),
                "pushRefused" => <Option<String> as Check>::describe(rest),
                _ => None,
            },
        }
    }
}
impl SubResult {
    fn rule_commits(x: &JsInt) -> Result<(), crate::types::check::SchemaError> {
        crate::types::check::num_at_least((*x).0.0, 0.0)?;
        Ok(())
    }
    fn rule_unpushed(x: &JsInt) -> Result<(), crate::types::check::SchemaError> {
        crate::types::check::num_at_least((*x).0.0, 0.0)?;
        Ok(())
    }
    fn rule_changed(x: &JsInt) -> Result<(), crate::types::check::SchemaError> {
        crate::types::check::num_at_least((*x).0.0, 0.0)?;
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SubSpawnBranch {
    pub branch: String,
    pub sha: String,
    pub ms: JsInt,
}

impl Check for SubSpawnBranch {
    /// Rules: ms: >= 0.
    fn check(&self) -> Result<(), SchemaError> {
        crate::types::check::num_at_least(self.ms.0.0, 0.0).map_err(|e| e.under("ms"))?;
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        match obj.get("branch") { None => out.push(crate::types::check::missing::<Self>("branch")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("branch"))), } } }
        match obj.get("sha") { None => out.push(crate::types::check::missing::<Self>("sha")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("sha"))), } } }
        match obj.get("ms") { None => out.push(crate::types::check::missing::<Self>("ms")), Some(v) => { match crate::types::check::read::<JsInt>(v) { Ok(x) => { if let Err(e) = Self::rule_ms(&x) { out.push(e.under("ms")); } }, Err(es) => out.extend(es.into_iter().map(|e| e.under("ms"))), } } }
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "branch" => <String as Check>::describe(rest),
                "sha" => <String as Check>::describe(rest),
                "ms" => <i64 as Check>::describe(rest),
                _ => None,
            },
        }
    }
}
impl SubSpawnBranch {
    fn rule_ms(x: &JsInt) -> Result<(), crate::types::check::SchemaError> {
        crate::types::check::num_at_least((*x).0.0, 0.0)?;
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SubSpawnMachine {
    pub since: JsNumber,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub id: Option<String>,
    pub states: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub ms: Option<JsInt>,
}

impl Check for SubSpawnMachine {
    /// Rules: ms: >= 0.
    fn check(&self) -> Result<(), SchemaError> {
        if let Some(v) = self.ms {
            crate::types::check::num_at_least(v.0.0, 0.0).map_err(|e| e.under("ms"))?;
        }
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        match obj.get("since") { None => out.push(crate::types::check::missing::<Self>("since")), Some(v) => { match crate::types::check::read::<JsNumber>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("since"))), } } }
        if let Some(v) = obj.get("id") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("id"))), } }
        match obj.get("states") { None => out.push(crate::types::check::missing::<Self>("states")), Some(v) => { match crate::types::check::read::<Vec<String>>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("states"))), } } }
        if let Some(v) = obj.get("ms") { match crate::types::check::read_present::<JsInt>(v) { Ok(x) => { if let Err(e) = Self::rule_ms(&x) { out.push(e.under("ms")); } }, Err(es) => out.extend(es.into_iter().map(|e| e.under("ms"))), } }
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "since" => <JsNumber as Check>::describe(rest),
                "id" => <Option<String> as Check>::describe(rest),
                "states" => <Vec<String> as Check>::describe(rest),
                "ms" => <Option<i64> as Check>::describe(rest),
                _ => None,
            },
        }
    }
}
impl SubSpawnMachine {
    fn rule_ms(x: &Option<JsInt>) -> Result<(), crate::types::check::SchemaError> {
        if let Some(v) = (*x) {
            crate::types::check::num_at_least(v.0.0, 0.0)?;
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SubSpawnFiles {
    pub since: JsNumber,
    pub paths: Vec<String>,
    pub bytes: JsInt,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub ms: Option<JsInt>,
}

impl Check for SubSpawnFiles {
    /// Rules: bytes: >= 0; ms: >= 0.
    fn check(&self) -> Result<(), SchemaError> {
        crate::types::check::num_at_least(self.bytes.0.0, 0.0).map_err(|e| e.under("bytes"))?;
        if let Some(v) = self.ms {
            crate::types::check::num_at_least(v.0.0, 0.0).map_err(|e| e.under("ms"))?;
        }
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        match obj.get("since") { None => out.push(crate::types::check::missing::<Self>("since")), Some(v) => { match crate::types::check::read::<JsNumber>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("since"))), } } }
        match obj.get("paths") { None => out.push(crate::types::check::missing::<Self>("paths")), Some(v) => { match crate::types::check::read::<Vec<String>>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("paths"))), } } }
        match obj.get("bytes") { None => out.push(crate::types::check::missing::<Self>("bytes")), Some(v) => { match crate::types::check::read::<JsInt>(v) { Ok(x) => { if let Err(e) = Self::rule_bytes(&x) { out.push(e.under("bytes")); } }, Err(es) => out.extend(es.into_iter().map(|e| e.under("bytes"))), } } }
        if let Some(v) = obj.get("ms") { match crate::types::check::read_present::<JsInt>(v) { Ok(x) => { if let Err(e) = Self::rule_ms(&x) { out.push(e.under("ms")); } }, Err(es) => out.extend(es.into_iter().map(|e| e.under("ms"))), } }
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "since" => <JsNumber as Check>::describe(rest),
                "paths" => <Vec<String> as Check>::describe(rest),
                "bytes" => <i64 as Check>::describe(rest),
                "ms" => <Option<i64> as Check>::describe(rest),
                _ => None,
            },
        }
    }
}
impl SubSpawnFiles {
    fn rule_bytes(x: &JsInt) -> Result<(), crate::types::check::SchemaError> {
        crate::types::check::num_at_least((*x).0.0, 0.0)?;
        Ok(())
    }
    fn rule_ms(x: &Option<JsInt>) -> Result<(), crate::types::check::SchemaError> {
        if let Some(v) = (*x) {
            crate::types::check::num_at_least(v.0.0, 0.0)?;
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SubSpawnFailedStep {
    #[serde(rename = "machine")]
    Machine,
    #[serde(rename = "files")]
    Files,
}

impl Check for SubSpawnFailedStep {
    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        crate::types::check::leaf(path, crate::types::check::Zod::Options(&["machine", "files"]))
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SubSpawnFailed {
    pub step: SubSpawnFailedStep,
    pub said: String,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub fix: Option<String>,
}

impl Check for SubSpawnFailed {
    fn check(&self) -> Result<(), SchemaError> {
        self.step.check().map_err(|e| e.under("step"))?;
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        match obj.get("step") { None => out.push(crate::types::check::missing::<Self>("step")), Some(v) => { match crate::types::check::read::<SubSpawnFailedStep>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("step"))), } } }
        match obj.get("said") { None => out.push(crate::types::check::missing::<Self>("said")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("said"))), } } }
        if let Some(v) = obj.get("fix") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("fix"))), } }
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "step" => <SubSpawnFailedStep as Check>::describe(rest),
                "said" => <String as Check>::describe(rest),
                "fix" => <Option<String> as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

/// TS: packages/protocol/src/views/sub-row.ts:34 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SubSpawn {
    pub started_at: JsNumber,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub branch: Option<SubSpawnBranch>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub machine: Option<SubSpawnMachine>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub files: Option<SubSpawnFiles>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub ready_at: Option<JsNumber>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub failed: Option<SubSpawnFailed>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub uncommitted: Option<JsInt>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub asked: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub not_sent: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub replaced: Option<Vec<String>>,
    pub log: Vec<String>,
}

impl Check for SubSpawn {
    /// Rules: uncommitted: >= 0.
    /// Nested checks: branch, machine, files, failed.
    fn check(&self) -> Result<(), SchemaError> {
        self.branch.check().map_err(|e| e.under("branch"))?;
        self.machine.check().map_err(|e| e.under("machine"))?;
        self.files.check().map_err(|e| e.under("files"))?;
        self.failed.check().map_err(|e| e.under("failed"))?;
        if let Some(v) = self.uncommitted {
            crate::types::check::num_at_least(v.0.0, 0.0).map_err(|e| e.under("uncommitted"))?;
        }
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        match obj.get("startedAt") { None => out.push(crate::types::check::missing::<Self>("startedAt")), Some(v) => { match crate::types::check::read::<JsNumber>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("startedAt"))), } } }
        if let Some(v) = obj.get("branch") { match crate::types::check::read_present::<SubSpawnBranch>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("branch"))), } }
        if let Some(v) = obj.get("machine") { match crate::types::check::read_present::<SubSpawnMachine>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("machine"))), } }
        if let Some(v) = obj.get("files") { match crate::types::check::read_present::<SubSpawnFiles>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("files"))), } }
        if let Some(v) = obj.get("readyAt") { match crate::types::check::read_present::<JsNumber>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("readyAt"))), } }
        if let Some(v) = obj.get("failed") { match crate::types::check::read_present::<SubSpawnFailed>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("failed"))), } }
        if let Some(v) = obj.get("uncommitted") { match crate::types::check::read_present::<JsInt>(v) { Ok(x) => { if let Err(e) = Self::rule_uncommitted(&x) { out.push(e.under("uncommitted")); } }, Err(es) => out.extend(es.into_iter().map(|e| e.under("uncommitted"))), } }
        if let Some(v) = obj.get("asked") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("asked"))), } }
        if let Some(v) = obj.get("notSent") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("notSent"))), } }
        if let Some(v) = obj.get("replaced") { match crate::types::check::read_present::<Vec<String>>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("replaced"))), } }
        match obj.get("log") { None => out.push(crate::types::check::missing::<Self>("log")), Some(v) => { match crate::types::check::read::<Vec<String>>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("log"))), } } }
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "startedAt" => <JsNumber as Check>::describe(rest),
                "branch" => <Option<SubSpawnBranch> as Check>::describe(rest),
                "machine" => <Option<SubSpawnMachine> as Check>::describe(rest),
                "files" => <Option<SubSpawnFiles> as Check>::describe(rest),
                "readyAt" => <Option<JsNumber> as Check>::describe(rest),
                "failed" => <Option<SubSpawnFailed> as Check>::describe(rest),
                "uncommitted" => <Option<i64> as Check>::describe(rest),
                "asked" => <Option<String> as Check>::describe(rest),
                "notSent" => <Option<String> as Check>::describe(rest),
                "replaced" => <Option<Vec<String>> as Check>::describe(rest),
                "log" => <Vec<String> as Check>::describe(rest),
                _ => None,
            },
        }
    }
}
impl SubSpawn {
    fn rule_uncommitted(x: &Option<JsInt>) -> Result<(), crate::types::check::SchemaError> {
        if let Some(v) = (*x) {
            crate::types::check::num_at_least(v.0.0, 0.0)?;
        }
        Ok(())
    }
}

/// TS: packages/protocol/src/views/sub-row.ts:62 at a3b870a6e.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SubState {
    #[serde(rename = "waiting")]
    Waiting,
    #[serde(rename = "starting")]
    Starting,
    #[serde(rename = "running")]
    Running,
    #[serde(rename = "napping")]
    Napping,
    #[serde(rename = "released")]
    Released,
}

impl Check for SubState {
    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        crate::types::check::leaf(path, crate::types::check::Zod::Options(&["waiting", "starting", "running", "napping", "released"]))
    }
}

/// TS: packages/protocol/src/views/sub-row.ts:66 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SubHeld {
    pub files: JsInt,
    pub commits: JsInt,
}

impl Check for SubHeld {
    /// Rules: files: >= 0; commits: >= 0.
    fn check(&self) -> Result<(), SchemaError> {
        crate::types::check::num_at_least(self.files.0.0, 0.0).map_err(|e| e.under("files"))?;
        crate::types::check::num_at_least(self.commits.0.0, 0.0).map_err(|e| e.under("commits"))?;
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        match obj.get("files") { None => out.push(crate::types::check::missing::<Self>("files")), Some(v) => { match crate::types::check::read::<JsInt>(v) { Ok(x) => { if let Err(e) = Self::rule_files(&x) { out.push(e.under("files")); } }, Err(es) => out.extend(es.into_iter().map(|e| e.under("files"))), } } }
        match obj.get("commits") { None => out.push(crate::types::check::missing::<Self>("commits")), Some(v) => { match crate::types::check::read::<JsInt>(v) { Ok(x) => { if let Err(e) = Self::rule_commits(&x) { out.push(e.under("commits")); } }, Err(es) => out.extend(es.into_iter().map(|e| e.under("commits"))), } } }
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "files" => <i64 as Check>::describe(rest),
                "commits" => <i64 as Check>::describe(rest),
                _ => None,
            },
        }
    }
}
impl SubHeld {
    fn rule_files(x: &JsInt) -> Result<(), crate::types::check::SchemaError> {
        crate::types::check::num_at_least((*x).0.0, 0.0)?;
        Ok(())
    }
    fn rule_commits(x: &JsInt) -> Result<(), crate::types::check::SchemaError> {
        crate::types::check::num_at_least((*x).0.0, 0.0)?;
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SubRowSize {
    pub cpu: JsInt,
    pub mem_mb: JsInt,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub name: Option<String>,
}

impl Check for SubRowSize {
    fn check(&self) -> Result<(), SchemaError> {
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        match obj.get("cpu") { None => out.push(crate::types::check::missing::<Self>("cpu")), Some(v) => { match crate::types::check::read::<JsInt>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("cpu"))), } } }
        match obj.get("memMb") { None => out.push(crate::types::check::missing::<Self>("memMb")), Some(v) => { match crate::types::check::read::<JsInt>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("memMb"))), } } }
        if let Some(v) = obj.get("name") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("name"))), } }
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "cpu" => <i64 as Check>::describe(rest),
                "memMb" => <i64 as Check>::describe(rest),
                "name" => <Option<String> as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

/// TS: packages/protocol/src/views/sub-row.ts:69 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SubRow {
    pub state: SubState,
    pub on: String,
    pub machine: String,
    pub size: SubRowSize,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub rate_usd_per_hour: Option<JsNumber>,
    pub branch: String,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub result: Option<SubResult>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub held: Option<SubHeld>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub delete_at: Option<JsNumber>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub released_at: Option<JsNumber>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub cloud_gone: Option<True>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub session_not_kept: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub spawn: Option<SubSpawn>,
}

impl Check for SubRow {
    /// Nested checks: state, size, result, held, spawn.
    fn check(&self) -> Result<(), SchemaError> {
        self.state.check().map_err(|e| e.under("state"))?;
        self.size.check().map_err(|e| e.under("size"))?;
        self.result.check().map_err(|e| e.under("result"))?;
        self.held.check().map_err(|e| e.under("held"))?;
        self.spawn.check().map_err(|e| e.under("spawn"))?;
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        match obj.get("state") { None => out.push(crate::types::check::missing::<Self>("state")), Some(v) => { match crate::types::check::read::<SubState>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("state"))), } } }
        match obj.get("on") { None => out.push(crate::types::check::missing::<Self>("on")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("on"))), } } }
        match obj.get("machine") { None => out.push(crate::types::check::missing::<Self>("machine")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("machine"))), } } }
        match obj.get("size") { None => out.push(crate::types::check::missing::<Self>("size")), Some(v) => { match crate::types::check::read::<SubRowSize>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("size"))), } } }
        if let Some(v) = obj.get("rateUsdPerHour") { match crate::types::check::read_present::<JsNumber>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("rateUsdPerHour"))), } }
        match obj.get("branch") { None => out.push(crate::types::check::missing::<Self>("branch")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("branch"))), } } }
        if let Some(v) = obj.get("result") { match crate::types::check::read_present::<SubResult>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("result"))), } }
        if let Some(v) = obj.get("held") { match crate::types::check::read_present::<SubHeld>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("held"))), } }
        if let Some(v) = obj.get("deleteAt") { match crate::types::check::read_present::<JsNumber>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("deleteAt"))), } }
        if let Some(v) = obj.get("releasedAt") { match crate::types::check::read_present::<JsNumber>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("releasedAt"))), } }
        if let Some(v) = obj.get("cloudGone") { match crate::types::check::read_present::<True>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("cloudGone"))), } }
        if let Some(v) = obj.get("sessionNotKept") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("sessionNotKept"))), } }
        if let Some(v) = obj.get("spawn") { match crate::types::check::read_present::<SubSpawn>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("spawn"))), } }
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "state" => <SubState as Check>::describe(rest),
                "on" => <String as Check>::describe(rest),
                "machine" => <String as Check>::describe(rest),
                "size" => <SubRowSize as Check>::describe(rest),
                "rateUsdPerHour" => <Option<JsNumber> as Check>::describe(rest),
                "branch" => <String as Check>::describe(rest),
                "result" => <Option<SubResult> as Check>::describe(rest),
                "held" => <Option<SubHeld> as Check>::describe(rest),
                "deleteAt" => <Option<JsNumber> as Check>::describe(rest),
                "releasedAt" => <Option<JsNumber> as Check>::describe(rest),
                "cloudGone" => None,
                "sessionNotKept" => <Option<String> as Check>::describe(rest),
                "spawn" => <Option<SubSpawn> as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

// ---- Hand-written below: the functions of the unit.

// views/sub-row.ts:58 at a3b870a6e
// export const subSpawning = (spawn: SubSpawn | undefined): boolean => spawn !== undefined && spawn.readyAt === undefined && spawn.failed === undefined;
pub fn sub_spawning(spawn: Option<&SubSpawn>) -> bool {
    todo!()
}

// views/sub-row.ts:93 at a3b870a6e
// export const subAllPushed = (r: SubResult): boolean => r.readFailed === undefined && r.unpushed === 0 && r.changed === 0;
pub fn sub_all_pushed(r: &SubResult) -> bool {
    todo!()
}

// views/sub-row.ts:97 at a3b870a6e
// export const subTurnGone = (sub: { releasedAt?: number } | undefined, startedAt: number | undefined): boolean =>
//   sub?.releasedAt !== undefined && (startedAt === undefined || startedAt < sub.releasedAt);
/// `released_at` is the sub-thread's, where there is one; `started_at` the turn's start.
pub fn sub_turn_gone(released_at: Option<f64>, started_at: Option<f64>) -> bool {
    todo!()
}
