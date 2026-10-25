// SPDX-License-Identifier: AGPL-3.0-only
//! The replies and words of wire/machine-link.ts that wsp-frames also holds, read as zod reads them: zod's numbers,
//! `null` refused on an optional key, zod's issues in zod's order. See spec/machine_link.md.

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::machine_link::{SysSampleDisk, SysSampleMem};
use crate::types::check::{leaf, not_object, Check, SchemaError, Zod};
use crate::types::js_number::{JsInt, JsNumber};
use crate::types::keys;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ContainerPlacedBy {
    #[serde(rename = "label")]
    Label,
    #[serde(rename = "compose")]
    Compose,
}
impl Check for ContainerPlacedBy {
    fn describe(path: &[Value]) -> Option<Zod> {
        leaf(path, Zod::Options(&["label", "compose"]))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TranscriptVoice {
    #[serde(rename = "person")]
    Person,
    #[serde(rename = "agent")]
    Agent,
    #[serde(rename = "tool")]
    Tool,
}
impl Check for TranscriptVoice {
    fn describe(path: &[Value]) -> Option<Zod> {
        leaf(path, Zod::Options(&["person", "agent", "tool"]))
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProcCost {
    pub cpu_ms: JsNumber,
    pub held: JsInt,
}
impl Check for ProcCost {
    fn check(&self) -> Result<(), SchemaError> {
        Ok(())
    }

    fn pass(value: &Value) -> Option<Vec<SchemaError>> {
        let Value::Object(obj) = value else {
            return Some(vec![not_object(value)]);
        };
        let mut out = Vec::new();
        keys::req_key::<JsNumber>(obj, "cpuMs", &mut out);
        keys::req_key::<JsInt>(obj, "held", &mut out);
        Some(out)
    }

    fn describe(path: &[Value]) -> Option<Zod> {
        match path.split_first() {
            None => Some(Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "cpuMs" => <JsNumber as Check>::describe(rest),
                "held" => <JsInt as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SysPoint {
    pub at: JsNumber,
    pub cpu: JsNumber,
    pub load1: JsNumber,
    pub mem: SysSampleMem,
    pub disk: SysSampleDisk,
}
impl Check for SysPoint {
    fn check(&self) -> Result<(), SchemaError> {
        self.mem.check().map_err(|e| e.under("mem"))?;
        self.disk.check().map_err(|e| e.under("disk"))?;
        Ok(())
    }

    fn pass(value: &Value) -> Option<Vec<SchemaError>> {
        let Value::Object(obj) = value else {
            return Some(vec![not_object(value)]);
        };
        let mut out = Vec::new();
        keys::req_key::<JsNumber>(obj, "at", &mut out);
        keys::req_key::<JsNumber>(obj, "cpu", &mut out);
        keys::req_key::<JsNumber>(obj, "load1", &mut out);
        keys::req_key::<SysSampleMem>(obj, "mem", &mut out);
        keys::req_key::<SysSampleDisk>(obj, "disk", &mut out);
        Some(out)
    }

    fn describe(path: &[Value]) -> Option<Zod> {
        match path.split_first() {
            None => Some(Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "at" => <JsNumber as Check>::describe(rest),
                "cpu" => <JsNumber as Check>::describe(rest),
                "load1" => <JsNumber as Check>::describe(rest),
                "mem" => <SysSampleMem as Check>::describe(rest),
                "disk" => <SysSampleDisk as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SysHistoryReply {
    pub points: Vec<SysPoint>,
    pub step_ms: JsNumber,
    pub truncated: bool,
}
impl Check for SysHistoryReply {
    fn check(&self) -> Result<(), SchemaError> {
        self.points.check().map_err(|e| e.under("points"))?;
        Ok(())
    }

    fn pass(value: &Value) -> Option<Vec<SchemaError>> {
        let Value::Object(obj) = value else {
            return Some(vec![not_object(value)]);
        };
        let mut out = Vec::new();
        keys::req_key::<Vec<SysPoint>>(obj, "points", &mut out);
        keys::req_key::<JsNumber>(obj, "stepMs", &mut out);
        keys::req_key::<bool>(obj, "truncated", &mut out);
        Some(out)
    }

    fn describe(path: &[Value]) -> Option<Zod> {
        match path.split_first() {
            None => Some(Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "points" => <Vec<SysPoint> as Check>::describe(rest),
                "stepMs" => <JsNumber as Check>::describe(rest),
                "truncated" => <bool as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UsageTokens {
    pub input: JsNumber,
    pub output: JsNumber,
    pub cached: JsNumber,
    pub cache_write: JsNumber,
    pub reasoning: JsNumber,
}
impl Check for UsageTokens {
    fn check(&self) -> Result<(), SchemaError> {
        Ok(())
    }

    fn pass(value: &Value) -> Option<Vec<SchemaError>> {
        let Value::Object(obj) = value else {
            return Some(vec![not_object(value)]);
        };
        let mut out = Vec::new();
        keys::req_key::<JsNumber>(obj, "input", &mut out);
        keys::req_key::<JsNumber>(obj, "output", &mut out);
        keys::req_key::<JsNumber>(obj, "cached", &mut out);
        keys::req_key::<JsNumber>(obj, "cacheWrite", &mut out);
        keys::req_key::<JsNumber>(obj, "reasoning", &mut out);
        Some(out)
    }

    fn describe(path: &[Value]) -> Option<Zod> {
        match path.split_first() {
            None => Some(Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "input" => <JsNumber as Check>::describe(rest),
                "output" => <JsNumber as Check>::describe(rest),
                "cached" => <JsNumber as Check>::describe(rest),
                "cacheWrite" => <JsNumber as Check>::describe(rest),
                "reasoning" => <JsNumber as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UsageLimitWindow {
    pub used_percent: JsNumber,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub window_duration_mins: Option<JsNumber>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub resets_at: Option<JsNumber>,
}
impl Check for UsageLimitWindow {
    fn check(&self) -> Result<(), SchemaError> {
        Ok(())
    }

    fn pass(value: &Value) -> Option<Vec<SchemaError>> {
        let Value::Object(obj) = value else {
            return Some(vec![not_object(value)]);
        };
        let mut out = Vec::new();
        keys::req_key::<JsNumber>(obj, "usedPercent", &mut out);
        keys::opt_key::<JsNumber>(obj, "windowDurationMins", &mut out);
        keys::opt_key::<JsNumber>(obj, "resetsAt", &mut out);
        Some(out)
    }

    fn describe(path: &[Value]) -> Option<Zod> {
        match path.split_first() {
            None => Some(Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "usedPercent" => <JsNumber as Check>::describe(rest),
                "windowDurationMins" => <Option<JsNumber> as Check>::describe(rest),
                "resetsAt" => <Option<JsNumber> as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UsageLogRow {
    pub agent: String,
    pub session: String,
    pub at: JsNumber,
    pub model: String,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub folder: Option<String>,
    pub tokens: UsageTokens,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub cost: Option<JsNumber>,
}
impl Check for UsageLogRow {
    fn check(&self) -> Result<(), SchemaError> {
        self.tokens.check().map_err(|e| e.under("tokens"))?;
        Ok(())
    }

    fn pass(value: &Value) -> Option<Vec<SchemaError>> {
        let Value::Object(obj) = value else {
            return Some(vec![not_object(value)]);
        };
        let mut out = Vec::new();
        keys::req_key::<String>(obj, "agent", &mut out);
        keys::req_key::<String>(obj, "session", &mut out);
        keys::req_key::<JsNumber>(obj, "at", &mut out);
        keys::req_key::<String>(obj, "model", &mut out);
        keys::opt_key::<String>(obj, "folder", &mut out);
        keys::req_key::<UsageTokens>(obj, "tokens", &mut out);
        keys::opt_key::<JsNumber>(obj, "cost", &mut out);
        Some(out)
    }

    fn describe(path: &[Value]) -> Option<Zod> {
        match path.split_first() {
            None => Some(Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "agent" => <String as Check>::describe(rest),
                "session" => <String as Check>::describe(rest),
                "at" => <JsNumber as Check>::describe(rest),
                "model" => <String as Check>::describe(rest),
                "folder" => <Option<String> as Check>::describe(rest),
                "tokens" => <UsageTokens as Check>::describe(rest),
                "cost" => <Option<JsNumber> as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UsageLimitReading {
    pub agent: String,
    pub at: JsNumber,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub primary: Option<UsageLimitWindow>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub secondary: Option<UsageLimitWindow>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub plan_type: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub rate_limit_reached_type: Option<String>,
}
impl Check for UsageLimitReading {
    fn check(&self) -> Result<(), SchemaError> {
        self.primary.check().map_err(|e| e.under("primary"))?;
        self.secondary.check().map_err(|e| e.under("secondary"))?;
        Ok(())
    }

    fn pass(value: &Value) -> Option<Vec<SchemaError>> {
        let Value::Object(obj) = value else {
            return Some(vec![not_object(value)]);
        };
        let mut out = Vec::new();
        keys::req_key::<String>(obj, "agent", &mut out);
        keys::req_key::<JsNumber>(obj, "at", &mut out);
        keys::opt_key::<UsageLimitWindow>(obj, "primary", &mut out);
        keys::opt_key::<UsageLimitWindow>(obj, "secondary", &mut out);
        keys::opt_key::<String>(obj, "planType", &mut out);
        keys::opt_key::<String>(obj, "rateLimitReachedType", &mut out);
        Some(out)
    }

    fn describe(path: &[Value]) -> Option<Zod> {
        match path.split_first() {
            None => Some(Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "agent" => <String as Check>::describe(rest),
                "at" => <JsNumber as Check>::describe(rest),
                "primary" => <Option<UsageLimitWindow> as Check>::describe(rest),
                "secondary" => <Option<UsageLimitWindow> as Check>::describe(rest),
                "planType" => <Option<String> as Check>::describe(rest),
                "rateLimitReachedType" => <Option<String> as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UsageLogsReply {
    pub rows: Vec<UsageLogRow>,
    pub limits: Vec<UsageLimitReading>,
}
impl Check for UsageLogsReply {
    fn check(&self) -> Result<(), SchemaError> {
        self.rows.check().map_err(|e| e.under("rows"))?;
        self.limits.check().map_err(|e| e.under("limits"))?;
        Ok(())
    }

    fn pass(value: &Value) -> Option<Vec<SchemaError>> {
        let Value::Object(obj) = value else {
            return Some(vec![not_object(value)]);
        };
        let mut out = Vec::new();
        keys::req_key::<Vec<UsageLogRow>>(obj, "rows", &mut out);
        keys::req_key::<Vec<UsageLimitReading>>(obj, "limits", &mut out);
        Some(out)
    }

    fn describe(path: &[Value]) -> Option<Zod> {
        match path.split_first() {
            None => Some(Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "rows" => <Vec<UsageLogRow> as Check>::describe(rest),
                "limits" => <Vec<UsageLimitReading> as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TranscriptRow {
    pub id: String,
    pub cwd: String,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub branch: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub entrypoint: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub title: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub first_prompt: Option<String>,
    pub last_at: JsNumber,
    pub bytes: JsNumber,
}
impl Check for TranscriptRow {
    fn check(&self) -> Result<(), SchemaError> {
        Ok(())
    }

    fn pass(value: &Value) -> Option<Vec<SchemaError>> {
        let Value::Object(obj) = value else {
            return Some(vec![not_object(value)]);
        };
        let mut out = Vec::new();
        keys::req_key::<String>(obj, "id", &mut out);
        keys::req_key::<String>(obj, "cwd", &mut out);
        keys::opt_key::<String>(obj, "branch", &mut out);
        keys::opt_key::<String>(obj, "entrypoint", &mut out);
        keys::opt_key::<String>(obj, "title", &mut out);
        keys::opt_key::<String>(obj, "firstPrompt", &mut out);
        keys::req_key::<JsNumber>(obj, "lastAt", &mut out);
        keys::req_key::<JsNumber>(obj, "bytes", &mut out);
        Some(out)
    }

    fn describe(path: &[Value]) -> Option<Zod> {
        match path.split_first() {
            None => Some(Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "id" => <String as Check>::describe(rest),
                "cwd" => <String as Check>::describe(rest),
                "branch" => <Option<String> as Check>::describe(rest),
                "entrypoint" => <Option<String> as Check>::describe(rest),
                "title" => <Option<String> as Check>::describe(rest),
                "firstPrompt" => <Option<String> as Check>::describe(rest),
                "lastAt" => <JsNumber as Check>::describe(rest),
                "bytes" => <JsNumber as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TranscriptsListReply {
    pub rows: Vec<TranscriptRow>,
}
impl Check for TranscriptsListReply {
    fn check(&self) -> Result<(), SchemaError> {
        self.rows.check().map_err(|e| e.under("rows"))?;
        Ok(())
    }

    fn pass(value: &Value) -> Option<Vec<SchemaError>> {
        let Value::Object(obj) = value else {
            return Some(vec![not_object(value)]);
        };
        let mut out = Vec::new();
        keys::req_key::<Vec<TranscriptRow>>(obj, "rows", &mut out);
        Some(out)
    }

    fn describe(path: &[Value]) -> Option<Zod> {
        match path.split_first() {
            None => Some(Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "rows" => <Vec<TranscriptRow> as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TranscriptMessage {
    pub who: TranscriptVoice,
    pub text: String,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub tool: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub at: Option<JsNumber>,
}
impl Check for TranscriptMessage {
    fn check(&self) -> Result<(), SchemaError> {
        Ok(())
    }

    fn pass(value: &Value) -> Option<Vec<SchemaError>> {
        let Value::Object(obj) = value else {
            return Some(vec![not_object(value)]);
        };
        let mut out = Vec::new();
        keys::req_key::<TranscriptVoice>(obj, "who", &mut out);
        keys::req_key::<String>(obj, "text", &mut out);
        keys::opt_key::<String>(obj, "tool", &mut out);
        keys::opt_key::<JsNumber>(obj, "at", &mut out);
        Some(out)
    }

    fn describe(path: &[Value]) -> Option<Zod> {
        match path.split_first() {
            None => Some(Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "who" => <TranscriptVoice as Check>::describe(rest),
                "text" => <String as Check>::describe(rest),
                "tool" => <Option<String> as Check>::describe(rest),
                "at" => <Option<JsNumber> as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TranscriptsReadReply {
    pub found: bool,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub cwd: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub title: Option<String>,
    pub messages: Vec<TranscriptMessage>,
    pub earlier: JsNumber,
}
impl Check for TranscriptsReadReply {
    fn check(&self) -> Result<(), SchemaError> {
        self.messages.check().map_err(|e| e.under("messages"))?;
        Ok(())
    }

    fn pass(value: &Value) -> Option<Vec<SchemaError>> {
        let Value::Object(obj) = value else {
            return Some(vec![not_object(value)]);
        };
        let mut out = Vec::new();
        keys::req_key::<bool>(obj, "found", &mut out);
        keys::opt_key::<String>(obj, "cwd", &mut out);
        keys::opt_key::<String>(obj, "title", &mut out);
        keys::req_key::<Vec<TranscriptMessage>>(obj, "messages", &mut out);
        keys::req_key::<JsNumber>(obj, "earlier", &mut out);
        Some(out)
    }

    fn describe(path: &[Value]) -> Option<Zod> {
        match path.split_first() {
            None => Some(Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "found" => <bool as Check>::describe(rest),
                "cwd" => <Option<String> as Check>::describe(rest),
                "title" => <Option<String> as Check>::describe(rest),
                "messages" => <Vec<TranscriptMessage> as Check>::describe(rest),
                "earlier" => <JsNumber as Check>::describe(rest),
                _ => None,
            },
        }
    }
}
