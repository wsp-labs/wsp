// SPDX-License-Identifier: AGPL-3.0-only
//! Port of packages/protocol/src/sub-thread.ts at a3b870a6e: its zod shapes and plain constants, written by the generator.

use serde::{Deserialize, Serialize};

use crate::types::check::{Check, SchemaError};
use crate::types::js_number::JsInt;
use crate::types::js_number::JsNumber;

/// TS: packages/protocol/src/sub-thread.ts:19 at a3b870a6e.
pub const SUB_SEED_MAX_BYTES: u64 = 20_971_520;

/// TS: packages/protocol/src/sub-thread.ts:124 at a3b870a6e.
pub const SUB_SHALLOW_LINE: &str =
    "The history here is one commit deep: pass --depth to every git fetch, and run git fetch --deepen=<n> when you need older commits.";

/// TS: packages/protocol/src/sub-thread.ts:199 at a3b870a6e.
pub const SUB_TURN_GONE_LINE: &str =
    "that turn ran on a machine since deleted, so its files cannot be rewound; send to the thread to go on from where it stands.";

/// TS: packages/protocol/src/sub-thread.ts:300 at a3b870a6e.
pub const SUB_DROPPED_LINE: &str = "stopped while it waited for a machine, so no machine was started";

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SubOutSize {
    pub cpu: JsInt,
    pub mem_mb: JsInt,
}

impl Check for SubOutSize {
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
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "cpu" => <i64 as Check>::describe(rest),
                "memMb" => <i64 as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SubOutBase {
    pub branch: String,
    pub sha: String,
}

impl Check for SubOutBase {
    fn check(&self) -> Result<(), SchemaError> {
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        crate::types::keys::req_key::<String>(obj, "branch", &mut out);
        crate::types::keys::req_key::<String>(obj, "sha", &mut out);
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "branch" => <String as Check>::describe(rest),
                "sha" => <String as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SubOutSent {
    pub paths: Vec<String>,
    pub bytes: JsInt,
    pub missing: Vec<String>,
}

impl Check for SubOutSent {
    /// Rules: bytes: >= 0.
    fn check(&self) -> Result<(), SchemaError> {
        crate::types::check::num_at_least(self.bytes.0 .0, 0.0).map_err(|e| e.under("bytes"))?;
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        crate::types::keys::req_key::<Vec<String>>(obj, "paths", &mut out);
        crate::types::keys::req_key_with::<JsInt>(obj, "bytes", &mut out, Self::rule_bytes);
        crate::types::keys::req_key::<Vec<String>>(obj, "missing", &mut out);
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
impl SubOutSent {
    fn rule_bytes(x: &JsInt) -> Result<(), crate::types::check::SchemaError> {
        crate::types::check::num_at_least((*x).0 .0, 0.0)?;
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SubOutNotSent {
    pub uncommitted: JsInt,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub why: Option<String>,
}

impl Check for SubOutNotSent {
    /// Rules: uncommitted: >= 0.
    fn check(&self) -> Result<(), SchemaError> {
        crate::types::check::num_at_least(self.uncommitted.0 .0, 0.0).map_err(|e| e.under("uncommitted"))?;
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        crate::types::keys::req_key_with::<JsInt>(obj, "uncommitted", &mut out, Self::rule_uncommitted);
        crate::types::keys::opt_key::<String>(obj, "why", &mut out);
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "uncommitted" => <i64 as Check>::describe(rest),
                "why" => <Option<String> as Check>::describe(rest),
                _ => None,
            },
        }
    }
}
impl SubOutNotSent {
    fn rule_uncommitted(x: &JsInt) -> Result<(), crate::types::check::SchemaError> {
        crate::types::check::num_at_least((*x).0 .0, 0.0)?;
        Ok(())
    }
}

/// TS: packages/protocol/src/sub-thread.ts:24 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SubOut {
    pub on: String,
    pub machine: String,
    pub size: SubOutSize,
    pub rate_usd_per_hour: JsNumber,
    pub repo: String,
    pub base: SubOutBase,
    pub branch: String,
    pub sent: SubOutSent,
    pub not_sent: SubOutNotSent,
    pub delete_after_minutes: JsInt,
}

impl Check for SubOut {
    /// Rules: deleteAfterMinutes: >= 0.
    /// Nested checks: size, base, sent, notSent.
    fn check(&self) -> Result<(), SchemaError> {
        self.size.check().map_err(|e| e.under("size"))?;
        self.base.check().map_err(|e| e.under("base"))?;
        self.sent.check().map_err(|e| e.under("sent"))?;
        self.not_sent.check().map_err(|e| e.under("notSent"))?;
        crate::types::check::num_at_least(self.delete_after_minutes.0 .0, 0.0).map_err(|e| e.under("deleteAfterMinutes"))?;
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        crate::types::keys::req_key::<String>(obj, "on", &mut out);
        crate::types::keys::req_key::<String>(obj, "machine", &mut out);
        crate::types::keys::req_key::<SubOutSize>(obj, "size", &mut out);
        crate::types::keys::req_key::<JsNumber>(obj, "rateUsdPerHour", &mut out);
        crate::types::keys::req_key::<String>(obj, "repo", &mut out);
        crate::types::keys::req_key::<SubOutBase>(obj, "base", &mut out);
        crate::types::keys::req_key::<String>(obj, "branch", &mut out);
        crate::types::keys::req_key::<SubOutSent>(obj, "sent", &mut out);
        crate::types::keys::req_key::<SubOutNotSent>(obj, "notSent", &mut out);
        crate::types::keys::req_key_with::<JsInt>(obj, "deleteAfterMinutes", &mut out, Self::rule_delete_after_minutes);
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "on" => <String as Check>::describe(rest),
                "machine" => <String as Check>::describe(rest),
                "size" => <SubOutSize as Check>::describe(rest),
                "rateUsdPerHour" => <JsNumber as Check>::describe(rest),
                "repo" => <String as Check>::describe(rest),
                "base" => <SubOutBase as Check>::describe(rest),
                "branch" => <String as Check>::describe(rest),
                "sent" => <SubOutSent as Check>::describe(rest),
                "notSent" => <SubOutNotSent as Check>::describe(rest),
                "deleteAfterMinutes" => <i64 as Check>::describe(rest),
                _ => None,
            },
        }
    }
}
impl SubOut {
    fn rule_delete_after_minutes(x: &JsInt) -> Result<(), crate::types::check::SchemaError> {
        crate::types::check::num_at_least((*x).0 .0, 0.0)?;
        Ok(())
    }
}

/// TS: packages/protocol/src/sub-thread.ts:40 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SubStarted {
    pub out: SubOut,
    pub folder: String,
    pub preamble: String,
    pub harness: String,
}

impl Check for SubStarted {
    /// Nested checks: out.
    fn check(&self) -> Result<(), SchemaError> {
        self.out.check().map_err(|e| e.under("out"))?;
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        crate::types::keys::req_key::<SubOut>(obj, "out", &mut out);
        crate::types::keys::req_key::<String>(obj, "folder", &mut out);
        crate::types::keys::req_key::<String>(obj, "preamble", &mut out);
        crate::types::keys::req_key::<String>(obj, "harness", &mut out);
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "out" => <SubOut as Check>::describe(rest),
                "folder" => <String as Check>::describe(rest),
                "preamble" => <String as Check>::describe(rest),
                "harness" => <String as Check>::describe(rest),
                _ => None,
            },
        }
    }
}
