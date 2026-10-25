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
        crate::types::check::num_at_least(self.commits.0 .0, 0.0).map_err(|e| e.under("commits"))?;
        crate::types::check::num_at_least(self.unpushed.0 .0, 0.0).map_err(|e| e.under("unpushed"))?;
        crate::types::check::num_at_least(self.changed.0 .0, 0.0).map_err(|e| e.under("changed"))?;
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        crate::types::keys::req_key::<String>(obj, "branch", &mut out);
        crate::types::keys::req_key::<Option<String>>(obj, "pushedSha", &mut out);
        crate::types::keys::req_key_with::<JsInt>(obj, "commits", &mut out, Self::rule_commits);
        crate::types::keys::req_key_with::<JsInt>(obj, "unpushed", &mut out, Self::rule_unpushed);
        crate::types::keys::req_key_with::<JsInt>(obj, "changed", &mut out, Self::rule_changed);
        crate::types::keys::opt_key::<String>(obj, "onBranch", &mut out);
        crate::types::keys::opt_key::<True>(obj, "norepo", &mut out);
        crate::types::keys::opt_key::<String>(obj, "readFailed", &mut out);
        crate::types::keys::req_key::<String>(obj, "readAt", &mut out);
        crate::types::keys::opt_key::<String>(obj, "pushRefused", &mut out);
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
        crate::types::check::num_at_least(x.0 .0, 0.0)?;
        Ok(())
    }
    fn rule_unpushed(x: &JsInt) -> Result<(), crate::types::check::SchemaError> {
        crate::types::check::num_at_least(x.0 .0, 0.0)?;
        Ok(())
    }
    fn rule_changed(x: &JsInt) -> Result<(), crate::types::check::SchemaError> {
        crate::types::check::num_at_least(x.0 .0, 0.0)?;
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
        crate::types::check::num_at_least(self.ms.0 .0, 0.0).map_err(|e| e.under("ms"))?;
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        crate::types::keys::req_key::<String>(obj, "branch", &mut out);
        crate::types::keys::req_key::<String>(obj, "sha", &mut out);
        crate::types::keys::req_key_with::<JsInt>(obj, "ms", &mut out, Self::rule_ms);
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
        crate::types::check::num_at_least(x.0 .0, 0.0)?;
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
            crate::types::check::num_at_least(v.0 .0, 0.0).map_err(|e| e.under("ms"))?;
        }
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        crate::types::keys::req_key::<JsNumber>(obj, "since", &mut out);
        crate::types::keys::opt_key::<String>(obj, "id", &mut out);
        crate::types::keys::req_key::<Vec<String>>(obj, "states", &mut out);
        crate::types::keys::opt_key_with::<JsInt>(obj, "ms", &mut out, Self::rule_ms);
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
        if let Some(v) = *x {
            crate::types::check::num_at_least(v.0 .0, 0.0)?;
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
        crate::types::check::num_at_least(self.bytes.0 .0, 0.0).map_err(|e| e.under("bytes"))?;
        if let Some(v) = self.ms {
            crate::types::check::num_at_least(v.0 .0, 0.0).map_err(|e| e.under("ms"))?;
        }
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        crate::types::keys::req_key::<JsNumber>(obj, "since", &mut out);
        crate::types::keys::req_key::<Vec<String>>(obj, "paths", &mut out);
        crate::types::keys::req_key_with::<JsInt>(obj, "bytes", &mut out, Self::rule_bytes);
        crate::types::keys::opt_key_with::<JsInt>(obj, "ms", &mut out, Self::rule_ms);
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
        crate::types::check::num_at_least(x.0 .0, 0.0)?;
        Ok(())
    }
    fn rule_ms(x: &Option<JsInt>) -> Result<(), crate::types::check::SchemaError> {
        if let Some(v) = *x {
            crate::types::check::num_at_least(v.0 .0, 0.0)?;
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
        crate::types::keys::req_key::<SubSpawnFailedStep>(obj, "step", &mut out);
        crate::types::keys::req_key::<String>(obj, "said", &mut out);
        crate::types::keys::opt_key::<String>(obj, "fix", &mut out);
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
            crate::types::check::num_at_least(v.0 .0, 0.0).map_err(|e| e.under("uncommitted"))?;
        }
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        crate::types::keys::req_key::<JsNumber>(obj, "startedAt", &mut out);
        crate::types::keys::opt_key::<SubSpawnBranch>(obj, "branch", &mut out);
        crate::types::keys::opt_key::<SubSpawnMachine>(obj, "machine", &mut out);
        crate::types::keys::opt_key::<SubSpawnFiles>(obj, "files", &mut out);
        crate::types::keys::opt_key::<JsNumber>(obj, "readyAt", &mut out);
        crate::types::keys::opt_key::<SubSpawnFailed>(obj, "failed", &mut out);
        crate::types::keys::opt_key_with::<JsInt>(obj, "uncommitted", &mut out, Self::rule_uncommitted);
        crate::types::keys::opt_key::<String>(obj, "asked", &mut out);
        crate::types::keys::opt_key::<String>(obj, "notSent", &mut out);
        crate::types::keys::opt_key::<Vec<String>>(obj, "replaced", &mut out);
        crate::types::keys::req_key::<Vec<String>>(obj, "log", &mut out);
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
        if let Some(v) = *x {
            crate::types::check::num_at_least(v.0 .0, 0.0)?;
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
        crate::types::check::num_at_least(self.files.0 .0, 0.0).map_err(|e| e.under("files"))?;
        crate::types::check::num_at_least(self.commits.0 .0, 0.0).map_err(|e| e.under("commits"))?;
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        crate::types::keys::req_key_with::<JsInt>(obj, "files", &mut out, Self::rule_files);
        crate::types::keys::req_key_with::<JsInt>(obj, "commits", &mut out, Self::rule_commits);
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
        crate::types::check::num_at_least(x.0 .0, 0.0)?;
        Ok(())
    }
    fn rule_commits(x: &JsInt) -> Result<(), crate::types::check::SchemaError> {
        crate::types::check::num_at_least(x.0 .0, 0.0)?;
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
        crate::types::keys::req_key::<JsInt>(obj, "cpu", &mut out);
        crate::types::keys::req_key::<JsInt>(obj, "memMb", &mut out);
        crate::types::keys::opt_key::<String>(obj, "name", &mut out);
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
        crate::types::keys::req_key::<SubState>(obj, "state", &mut out);
        crate::types::keys::req_key::<String>(obj, "on", &mut out);
        crate::types::keys::req_key::<String>(obj, "machine", &mut out);
        crate::types::keys::req_key::<SubRowSize>(obj, "size", &mut out);
        crate::types::keys::opt_key::<JsNumber>(obj, "rateUsdPerHour", &mut out);
        crate::types::keys::req_key::<String>(obj, "branch", &mut out);
        crate::types::keys::opt_key::<SubResult>(obj, "result", &mut out);
        crate::types::keys::opt_key::<SubHeld>(obj, "held", &mut out);
        crate::types::keys::opt_key::<JsNumber>(obj, "deleteAt", &mut out);
        crate::types::keys::opt_key::<JsNumber>(obj, "releasedAt", &mut out);
        crate::types::keys::opt_key::<True>(obj, "cloudGone", &mut out);
        crate::types::keys::opt_key::<String>(obj, "sessionNotKept", &mut out);
        crate::types::keys::opt_key::<SubSpawn>(obj, "spawn", &mut out);
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
    spawn.is_some_and(|s| s.ready_at.is_none() && s.failed.is_none())
}

// views/sub-row.ts:93 at a3b870a6e
// export const subAllPushed = (r: SubResult): boolean => r.readFailed === undefined && r.unpushed === 0 && r.changed === 0;
pub fn sub_all_pushed(r: &SubResult) -> bool {
    r.read_failed.is_none() && r.unpushed.get() == 0 && r.changed.get() == 0
}

// views/sub-row.ts:97 at a3b870a6e
// export const subTurnGone = (sub: { releasedAt?: number } | undefined, startedAt: number | undefined): boolean =>
//   sub?.releasedAt !== undefined && (startedAt === undefined || startedAt < sub.releasedAt);
/// `released_at` is the sub-thread's, where there is one; `started_at` the turn's start.
pub fn sub_turn_gone(released_at: Option<f64>, started_at: Option<f64>) -> bool {
    released_at.is_some_and(|released| started_at.is_none_or(|started| started < released))
}
