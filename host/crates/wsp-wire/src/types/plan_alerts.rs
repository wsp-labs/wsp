// SPDX-License-Identifier: AGPL-3.0-only
//! Port of packages/protocol/src/plan-alerts.ts at a3b870a6e: its zod shapes and plain constants, written by the generator.

use serde::{Deserialize, Serialize};

use crate::types::check::{Check, SchemaError};
use crate::types::js_number::JsNumber;
use crate::types::usage::LimitKind;

/// TS: packages/protocol/src/plan-alerts.ts:10 at a3b870a6e.
pub const PLAN_ALERT_STEPS: &[u64] = &[70, 90];

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlanAlertLow {
    pub window: LimitKind,
    pub step: JsNumber,
}

impl PlanAlertLow {
    /// The `kind` key's value: the enum holding this struct writes and reads it as its tag.
    pub const KIND: &'static str = "low";
}

impl Check for PlanAlertLow {
    /// Nested checks: window.
    fn check(&self) -> Result<(), SchemaError> {
        self.window.check().map_err(|e| e.under("window"))?;
        Ok(())
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "window" => <LimitKind as Check>::describe(rest),
                "step" => <JsNumber as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlanAlertBlocked {}

impl PlanAlertBlocked {
    /// The `kind` key's value: the enum holding this struct writes and reads it as its tag.
    pub const KIND: &'static str = "blocked";
}

impl Check for PlanAlertBlocked {
    fn check(&self) -> Result<(), SchemaError> {
        Ok(())
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        if path.is_empty() { Some(crate::types::check::Zod::Word("object")) } else { None }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlanAlertBack {}

impl PlanAlertBack {
    /// The `kind` key's value: the enum holding this struct writes and reads it as its tag.
    pub const KIND: &'static str = "back";
}

impl Check for PlanAlertBack {
    fn check(&self) -> Result<(), SchemaError> {
        Ok(())
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        if path.is_empty() { Some(crate::types::check::Zod::Word("object")) } else { None }
    }
}

/// TS: packages/protocol/src/plan-alerts.ts:12 at a3b870a6e.
/// zod's discriminatedUnion on `kind`: each variant's struct leaves that key to this tag.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind")]
pub enum PlanAlert {
    #[serde(rename = "low")]
    Low(PlanAlertLow),
    #[serde(rename = "blocked")]
    Blocked(PlanAlertBlocked),
    #[serde(rename = "back")]
    Back(PlanAlertBack),
}

impl Check for PlanAlert {
    fn check(&self) -> Result<(), SchemaError> {
        match self {
            PlanAlert::Low(v) => v.check(),
            PlanAlert::Blocked(v) => v.check(),
            PlanAlert::Back(v) => v.check(),
        }
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        if path.is_empty() { return Some(crate::types::check::Zod::Word("object")); }
        if path.len() == 1 && path[0].as_str() == Some("kind") { return Some(crate::types::check::Zod::Tags("kind", &["low", "blocked", "back"])); }
        <PlanAlertLow as Check>::describe(path).or_else(|| <PlanAlertBlocked as Check>::describe(path)).or_else(|| <PlanAlertBack as Check>::describe(path))
    }
}

/// TS: packages/protocol/src/plan-alerts.ts:67 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UsageAlertEvent {
    pub key: String,
    pub agent: String,
    pub label: String,
    pub alert: PlanAlert,
}

impl UsageAlertEvent {
    /// The `type` key's value: the enum holding this struct writes and reads it as its tag.
    pub const TYPE: &'static str = "usage.alert";
}

impl Check for UsageAlertEvent {
    /// Nested checks: alert.
    fn check(&self) -> Result<(), SchemaError> {
        self.alert.check().map_err(|e| e.under("alert"))?;
        Ok(())
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "key" => <String as Check>::describe(rest),
                "agent" => <String as Check>::describe(rest),
                "label" => <String as Check>::describe(rest),
                "alert" => <PlanAlert as Check>::describe(rest),
                _ => None,
            },
        }
    }
}
