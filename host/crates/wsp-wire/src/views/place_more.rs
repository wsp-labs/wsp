// SPDX-License-Identifier: AGPL-3.0-only
//! The later shapes of packages/protocol/src/views/place.ts at a3b870a6e, apart only for the repo's file-size law; place.rs re-exports them.

#![allow(unused_imports)]

use serde::{Deserialize, Serialize};

use super::place::*;
use crate::types::agents_report::AgentSignInState;
use crate::types::check::{Check, SchemaError};
use crate::types::js_number::JsNumber;
use crate::types::recipe_file::RecipeFile;
use crate::views::place_kind::PlaceKind;
use crate::views::workspace::ProjectAddStage;
use crate::views::workspace::ProjectView;
use crate::views::workspace::WorkspaceAgents;
use crate::wire::capabilities::MachineSizeOffer;
use crate::wire::limits::PortForward;
use indexmap::IndexMap;
use crate::wire::limits::RelayPort;
use wsp_frames::True;
use crate::wire::capabilities::WorkspaceSize;

/// TS: packages/protocol/src/views/place.ts:432 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ComputerCap {
    pub threads: i64,
}

impl Check for ComputerCap {
    /// Rules: threads: >= 1.
    fn check(&self) -> Result<(), SchemaError> {
        crate::types::check::num_at_least(self.threads as f64, 1.0).map_err(|e| e.under("threads"))?;
        Ok(())
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "threads" => <i64 as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

/// TS: packages/protocol/src/views/place.ts:435 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CloudCap {
    pub machines: i64,
    pub spend_per_day_usd: JsNumber,
}

impl Check for CloudCap {
    /// Rules: machines: >= 1; spendPerDayUsd: >= 0.
    fn check(&self) -> Result<(), SchemaError> {
        crate::types::check::num_at_least(self.machines as f64, 1.0).map_err(|e| e.under("machines"))?;
        crate::types::check::num_at_least(self.spend_per_day_usd.0, 0.0).map_err(|e| e.under("spendPerDayUsd"))?;
        Ok(())
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "machines" => <i64 as Check>::describe(rest),
                "spendPerDayUsd" => <JsNumber as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

/// TS: packages/protocol/src/views/place.ts:437 at a3b870a6e.
/// A zod union: the first variant that parses wins, in this order.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum PlaceCap {
    ComputerCap(ComputerCap),
    CloudCap(CloudCap),
}

impl Check for PlaceCap {
    fn check(&self) -> Result<(), SchemaError> {
        match self {
            PlaceCap::ComputerCap(v) => v.check(),
            PlaceCap::CloudCap(v) => v.check(),
        }
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        if path.is_empty() { return Some(crate::types::check::Zod::Union); }
        <ComputerCap as Check>::describe(path).or_else(|| <CloudCap as Check>::describe(path))
    }
}

/// TS: packages/protocol/src/views/place.ts:440 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaceCapSet {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub threads: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub machines: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub spend_per_day_usd: Option<JsNumber>,
}

impl Check for PlaceCapSet {
    /// Rules: threads: >= 1; machines: >= 1; spendPerDayUsd: >= 0.
    fn check(&self) -> Result<(), SchemaError> {
        if let Some(v) = self.threads {
            crate::types::check::num_at_least(v as f64, 1.0).map_err(|e| e.under("threads"))?;
        }
        if let Some(v) = self.machines {
            crate::types::check::num_at_least(v as f64, 1.0).map_err(|e| e.under("machines"))?;
        }
        if let Some(v) = self.spend_per_day_usd {
            crate::types::check::num_at_least(v.0, 0.0).map_err(|e| e.under("spendPerDayUsd"))?;
        }
        Ok(())
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "threads" => <Option<i64> as Check>::describe(rest),
                "machines" => <Option<i64> as Check>::describe(rest),
                "spendPerDayUsd" => <Option<JsNumber> as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaceSettingsSpawn {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub spawn: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_machines: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_depth: Option<i64>,
}

impl Check for PlaceSettingsSpawn {
    /// Rules: maxMachines: >= 0; maxDepth: >= 1.
    fn check(&self) -> Result<(), SchemaError> {
        if let Some(v) = self.max_machines {
            crate::types::check::num_at_least(v as f64, 0.0).map_err(|e| e.under("maxMachines"))?;
        }
        if let Some(v) = self.max_depth {
            crate::types::check::num_at_least(v as f64, 1.0).map_err(|e| e.under("maxDepth"))?;
        }
        Ok(())
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "spawn" => <Option<bool> as Check>::describe(rest),
                "maxMachines" => <Option<i64> as Check>::describe(rest),
                "maxDepth" => <Option<i64> as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaceSettingsSize {
    pub cpu: i64,
    pub mem_mb: i64,
}

impl Check for PlaceSettingsSize {
    fn check(&self) -> Result<(), SchemaError> {
        Ok(())
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

/// TS: packages/protocol/src/views/place.ts:444 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaceSettings {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub threads: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub machines: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub spend_per_day_usd: Option<JsNumber>,
    #[serde(default, skip_serializing_if = "Option::is_none", with = "crate::types::check::double_option")]
    pub nap_ms: Option<Option<i64>>,
    #[serde(default, skip_serializing_if = "Option::is_none", with = "crate::types::check::double_option")]
    pub turn_limit_ms: Option<Option<i64>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub spawn: Option<PlaceSettingsSpawn>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub size: Option<PlaceSettingsSize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub delete_after_ms: Option<i64>,
}

impl Check for PlaceSettings {
    /// Rules: threads: >= 1; machines: >= 1; spendPerDayUsd: >= 0; napMs: >= 60000; napMs: <= 10800000; turnLimitMs: >= 3600000; turnLimitMs: <= 86400000; deleteAfterMs: >= 0; deleteAfterMs: <= 86400000.
    /// Nested checks: spawn, size.
    fn check(&self) -> Result<(), SchemaError> {
        if let Some(v) = self.threads {
            crate::types::check::num_at_least(v as f64, 1.0).map_err(|e| e.under("threads"))?;
        }
        if let Some(v) = self.machines {
            crate::types::check::num_at_least(v as f64, 1.0).map_err(|e| e.under("machines"))?;
        }
        if let Some(v) = self.spend_per_day_usd {
            crate::types::check::num_at_least(v.0, 0.0).map_err(|e| e.under("spendPerDayUsd"))?;
        }
        if let Some(Some(v)) = self.nap_ms {
            crate::types::check::num_at_least(v as f64, 60000.0).map_err(|e| e.under("napMs"))?;
        }
        if let Some(Some(v)) = self.nap_ms {
            crate::types::check::num_at_most(v as f64, 10800000.0).map_err(|e| e.under("napMs"))?;
        }
        if let Some(Some(v)) = self.turn_limit_ms {
            crate::types::check::num_at_least(v as f64, 3600000.0).map_err(|e| e.under("turnLimitMs"))?;
        }
        if let Some(Some(v)) = self.turn_limit_ms {
            crate::types::check::num_at_most(v as f64, 86400000.0).map_err(|e| e.under("turnLimitMs"))?;
        }
        self.spawn.check().map_err(|e| e.under("spawn"))?;
        self.size.check().map_err(|e| e.under("size"))?;
        if let Some(v) = self.delete_after_ms {
            crate::types::check::num_at_least(v as f64, 0.0).map_err(|e| e.under("deleteAfterMs"))?;
        }
        if let Some(v) = self.delete_after_ms {
            crate::types::check::num_at_most(v as f64, 86400000.0).map_err(|e| e.under("deleteAfterMs"))?;
        }
        Ok(())
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "threads" => <Option<i64> as Check>::describe(rest),
                "machines" => <Option<i64> as Check>::describe(rest),
                "spendPerDayUsd" => <Option<JsNumber> as Check>::describe(rest),
                "napMs" => <Option<Option<i64>> as Check>::describe(rest),
                "turnLimitMs" => <Option<Option<i64>> as Check>::describe(rest),
                "spawn" => <Option<PlaceSettingsSpawn> as Check>::describe(rest),
                "size" => <Option<PlaceSettingsSize> as Check>::describe(rest),
                "deleteAfterMs" => <Option<i64> as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

/// TS: packages/protocol/src/views/place.ts:458 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaceSettingsAsk {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub threads: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub machines: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub spend_per_day_usd: Option<JsNumber>,
    #[serde(default, skip_serializing_if = "Option::is_none", with = "crate::types::check::double_option")]
    pub nap_ms: Option<Option<i64>>,
    #[serde(default, skip_serializing_if = "Option::is_none", with = "crate::types::check::double_option")]
    pub turn_limit_ms: Option<Option<i64>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub spawn: Option<PlaceSettingsSpawn>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub size: Option<PlaceSettingsSize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub delete_after_ms: Option<i64>,
}

impl Check for PlaceSettingsAsk {
    /// Rules: threads: >= 1; machines: >= 1; spendPerDayUsd: >= 0; napMs: >= 60000; napMs: <= 10800000; turnLimitMs: >= 3600000; turnLimitMs: <= 86400000; deleteAfterMs: >= 0; deleteAfterMs: <= 86400000.
    fn check(&self) -> Result<(), SchemaError> {
        if let Some(v) = self.threads {
            crate::types::check::num_at_least(v as f64, 1.0).map_err(|e| e.under("threads"))?;
        }
        if let Some(v) = self.machines {
            crate::types::check::num_at_least(v as f64, 1.0).map_err(|e| e.under("machines"))?;
        }
        if let Some(v) = self.spend_per_day_usd {
            crate::types::check::num_at_least(v.0, 0.0).map_err(|e| e.under("spendPerDayUsd"))?;
        }
        if let Some(Some(v)) = self.nap_ms {
            crate::types::check::num_at_least(v as f64, 60000.0).map_err(|e| e.under("napMs"))?;
        }
        if let Some(Some(v)) = self.nap_ms {
            crate::types::check::num_at_most(v as f64, 10800000.0).map_err(|e| e.under("napMs"))?;
        }
        if let Some(Some(v)) = self.turn_limit_ms {
            crate::types::check::num_at_least(v as f64, 3600000.0).map_err(|e| e.under("turnLimitMs"))?;
        }
        if let Some(Some(v)) = self.turn_limit_ms {
            crate::types::check::num_at_most(v as f64, 86400000.0).map_err(|e| e.under("turnLimitMs"))?;
        }
        self.spawn.check().map_err(|e| e.under("spawn"))?;
        self.size.check().map_err(|e| e.under("size"))?;
        if let Some(v) = self.delete_after_ms {
            crate::types::check::num_at_least(v as f64, 0.0).map_err(|e| e.under("deleteAfterMs"))?;
        }
        if let Some(v) = self.delete_after_ms {
            crate::types::check::num_at_most(v as f64, 86400000.0).map_err(|e| e.under("deleteAfterMs"))?;
        }
        Ok(())
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "threads" => <Option<i64> as Check>::describe(rest),
                "machines" => <Option<i64> as Check>::describe(rest),
                "spendPerDayUsd" => <Option<JsNumber> as Check>::describe(rest),
                "napMs" => <Option<Option<i64>> as Check>::describe(rest),
                "turnLimitMs" => <Option<Option<i64>> as Check>::describe(rest),
                "spawn" => <Option<PlaceSettingsSpawn> as Check>::describe(rest),
                "size" => <Option<PlaceSettingsSize> as Check>::describe(rest),
                "deleteAfterMs" => <Option<i64> as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

/// TS: packages/protocol/src/views/place.ts:468 at a3b870a6e.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PlaceSettingWord {
    #[serde(rename = "threads")]
    Threads,
    #[serde(rename = "machines")]
    Machines,
    #[serde(rename = "spend")]
    Spend,
    #[serde(rename = "size")]
    Size,
    #[serde(rename = "delete-after")]
    DeleteAfter,
    #[serde(rename = "nap")]
    Nap,
    #[serde(rename = "turn-limit")]
    TurnLimit,
    #[serde(rename = "spawn")]
    Spawn,
    #[serde(rename = "max-depth")]
    MaxDepth,
}

impl Check for PlaceSettingWord {
    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        crate::types::check::leaf(path, crate::types::check::Zod::Options(&["threads", "machines", "spend", "size", "delete-after", "nap", "turn-limit", "spawn", "max-depth"]))
    }
}

/// TS: packages/protocol/src/views/place.ts:473 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CloudPlan {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub key: Option<String>,
    pub trial: bool,
    pub machines_at_once: i64,
    pub sizes: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub running: Option<i64>,
}

impl Check for CloudPlan {
    fn check(&self) -> Result<(), SchemaError> {
        Ok(())
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "key" => <Option<String> as Check>::describe(rest),
                "trial" => <bool as Check>::describe(rest),
                "machinesAtOnce" => <i64 as Check>::describe(rest),
                "sizes" => <Vec<String> as Check>::describe(rest),
                "running" => <Option<i64> as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

/// TS: packages/protocol/src/views/place.ts:484 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StartsWindow {
    pub used: i64,
    pub limit: i64,
}

impl Check for StartsWindow {
    fn check(&self) -> Result<(), SchemaError> {
        Ok(())
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "used" => <i64 as Check>::describe(rest),
                "limit" => <i64 as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

/// TS: packages/protocol/src/views/place.ts:488 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CloudStarts {
    pub minute: StartsWindow,
    pub hour: StartsWindow,
    pub day: StartsWindow,
}

impl Check for CloudStarts {
    /// Nested checks: minute, hour, day.
    fn check(&self) -> Result<(), SchemaError> {
        self.minute.check().map_err(|e| e.under("minute"))?;
        self.hour.check().map_err(|e| e.under("hour"))?;
        self.day.check().map_err(|e| e.under("day"))?;
        Ok(())
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "minute" => <StartsWindow as Check>::describe(rest),
                "hour" => <StartsWindow as Check>::describe(rest),
                "day" => <StartsWindow as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

/// TS: packages/protocol/src/views/place.ts:493 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CloudLimits {
    pub plan: CloudPlan,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub starts: Option<CloudStarts>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub credit_hours: Option<JsNumber>,
}

impl Check for CloudLimits {
    /// Nested checks: plan, starts.
    fn check(&self) -> Result<(), SchemaError> {
        self.plan.check().map_err(|e| e.under("plan"))?;
        self.starts.check().map_err(|e| e.under("starts"))?;
        Ok(())
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "plan" => <CloudPlan as Check>::describe(rest),
                "starts" => <Option<CloudStarts> as Check>::describe(rest),
                "creditHours" => <Option<JsNumber> as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

/// TS: packages/protocol/src/views/place.ts:497 at a3b870a6e.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum MacKind {
    #[serde(rename = "macbook")]
    Macbook,
    #[serde(rename = "imac")]
    Imac,
    #[serde(rename = "mac-mini")]
    MacMini,
    #[serde(rename = "mac-studio")]
    MacStudio,
    #[serde(rename = "mac-pro")]
    MacPro,
}

impl Check for MacKind {
    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        crate::types::check::leaf(path, crate::types::check::Zod::Options(&["macbook", "imac", "mac-mini", "mac-studio", "mac-pro"]))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PlaceViewEngine {
    #[serde(rename = "none")]
    None,
    #[serde(rename = "docker")]
    Docker,
    #[serde(rename = "podman")]
    Podman,
}

impl Check for PlaceViewEngine {
    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        crate::types::check::leaf(path, crate::types::check::Zod::Options(&["none", "docker", "podman"]))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PlaceViewBehindAct {
    #[serde(rename = "update")]
    Update,
    #[serde(rename = "install")]
    Install,
}

impl Check for PlaceViewBehindAct {
    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        crate::types::check::leaf(path, crate::types::check::Zod::Options(&["update", "install"]))
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaceViewBehind {
    pub word: String,
    pub fix: String,
    pub act: PlaceViewBehindAct,
}

impl Check for PlaceViewBehind {
    fn check(&self) -> Result<(), SchemaError> {
        self.act.check().map_err(|e| e.under("act"))?;
        Ok(())
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "word" => <String as Check>::describe(rest),
                "fix" => <String as Check>::describe(rest),
                "act" => <PlaceViewBehindAct as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaceViewForks {
    pub running: i64,
    pub room: i64,
}

impl Check for PlaceViewForks {
    fn check(&self) -> Result<(), SchemaError> {
        Ok(())
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "running" => <i64 as Check>::describe(rest),
                "room" => <i64 as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaceViewSize {
    pub cpu: i64,
    pub mem_mb: i64,
}

impl Check for PlaceViewSize {
    fn check(&self) -> Result<(), SchemaError> {
        Ok(())
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
pub struct PlaceViewSizeDefault {
    pub cpu: i64,
    pub mem_mb: i64,
}

impl Check for PlaceViewSizeDefault {
    fn check(&self) -> Result<(), SchemaError> {
        Ok(())
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

/// TS: packages/protocol/src/views/place.ts:513 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaceView {
    pub id: String,
    pub kind: PlaceKind,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mac: Option<MacKind>,
    pub default: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub os: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shape: Option<WorkspaceSize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub disk_free_bytes: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub engine: Option<PlaceViewEngine>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub copies: Option<CopyWord>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub present: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub takes_runtime: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub joined_at: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_seen_at: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub daemon_version: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub behind: Option<PlaceViewBehind>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub agents: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub agent_versions: Option<IndexMap<String, String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sign_ins: Option<IndexMap<String, AgentSignInState>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub logins: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rate_usd_per_hour: Option<JsNumber>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sizes: Option<Vec<MachineSizeOffer>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub forks: Option<PlaceViewForks>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub takes_forks: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub road: Option<PlaceRoad>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub home: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub uptime_ms: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reported_at: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dialled: Option<PlaceDialled>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub blocked: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tools_blocked: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub build: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub build_stopped: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub builds_images: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub setup: Option<PlaceSetup>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub applied: Option<PlaceApplied>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub picks: Option<RecipeFile>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub recipe: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sync: Option<PlaceSync>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cap: Option<PlaceCap>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cap_default: Option<PlaceCap>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub settings: Option<PlaceSettings>,
    #[serde(default, skip_serializing_if = "Option::is_none", with = "crate::types::check::double_option")]
    pub nap_ms: Option<Option<i64>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub nap_default: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none", with = "crate::types::check::double_option")]
    pub turn_limit_ms: Option<Option<i64>>,
    #[serde(default, skip_serializing_if = "Option::is_none", with = "crate::types::check::double_option")]
    pub turn_limit_default: Option<Option<i64>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub spawn: Option<WorkspaceAgents>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub spawn_default: Option<WorkspaceAgents>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub running: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub plan: Option<CloudPlan>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub starts: Option<CloudStarts>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub size: Option<PlaceViewSize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub size_default: Option<PlaceViewSizeDefault>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub delete_after_ms: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub credit_hours: Option<JsNumber>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub spent_today_usd: Option<JsNumber>,
}

impl Check for PlaceView {
    /// Rules: uptimeMs: >= 0; running: >= 0.
    /// Nested checks: kind, mac, shape, copies, behind, signIns, sizes, forks, road, dialled, setup, applied, picks, sync, cap, capDefault, settings, spawn, spawnDefault, plan, starts, size, sizeDefault.
    fn check(&self) -> Result<(), SchemaError> {
        self.kind.check().map_err(|e| e.under("kind"))?;
        self.mac.check().map_err(|e| e.under("mac"))?;
        self.engine.check().map_err(|e| e.under("engine"))?;
        self.behind.check().map_err(|e| e.under("behind"))?;
        self.sign_ins.check().map_err(|e| e.under("signIns"))?;
        self.sizes.check().map_err(|e| e.under("sizes"))?;
        self.forks.check().map_err(|e| e.under("forks"))?;
        if let Some(v) = self.uptime_ms {
            crate::types::check::num_at_least(v as f64, 0.0).map_err(|e| e.under("uptimeMs"))?;
        }
        self.picks.check().map_err(|e| e.under("picks"))?;
        self.cap.check().map_err(|e| e.under("cap"))?;
        self.cap_default.check().map_err(|e| e.under("capDefault"))?;
        self.settings.check().map_err(|e| e.under("settings"))?;
        self.spawn.check().map_err(|e| e.under("spawn"))?;
        self.spawn_default.check().map_err(|e| e.under("spawnDefault"))?;
        if let Some(v) = self.running {
            crate::types::check::num_at_least(v as f64, 0.0).map_err(|e| e.under("running"))?;
        }
        self.plan.check().map_err(|e| e.under("plan"))?;
        self.starts.check().map_err(|e| e.under("starts"))?;
        self.size.check().map_err(|e| e.under("size"))?;
        self.size_default.check().map_err(|e| e.under("sizeDefault"))?;
        Ok(())
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "id" => <String as Check>::describe(rest),
                "kind" => <PlaceKind as Check>::describe(rest),
                "name" => <String as Check>::describe(rest),
                "label" => <Option<String> as Check>::describe(rest),
                "mac" => <Option<MacKind> as Check>::describe(rest),
                "default" => <bool as Check>::describe(rest),
                "os" => <Option<String> as Check>::describe(rest),
                "shape" => None,
                "diskFreeBytes" => <Option<i64> as Check>::describe(rest),
                "engine" => <Option<PlaceViewEngine> as Check>::describe(rest),
                "copies" => None,
                "present" => <Option<bool> as Check>::describe(rest),
                "takesRuntime" => <Option<bool> as Check>::describe(rest),
                "joinedAt" => <Option<String> as Check>::describe(rest),
                "lastSeenAt" => <Option<String> as Check>::describe(rest),
                "daemonVersion" => <Option<i64> as Check>::describe(rest),
                "behind" => <Option<PlaceViewBehind> as Check>::describe(rest),
                "agents" => <Option<Vec<String>> as Check>::describe(rest),
                "agentVersions" => <Option<IndexMap<String, String>> as Check>::describe(rest),
                "signIns" => <Option<IndexMap<String, AgentSignInState>> as Check>::describe(rest),
                "logins" => <Option<String> as Check>::describe(rest),
                "rateUsdPerHour" => <Option<JsNumber> as Check>::describe(rest),
                "sizes" => <Option<Vec<MachineSizeOffer>> as Check>::describe(rest),
                "forks" => <Option<PlaceViewForks> as Check>::describe(rest),
                "takesForks" => <Option<bool> as Check>::describe(rest),
                "road" => None,
                "home" => <Option<String> as Check>::describe(rest),
                "uptimeMs" => <Option<i64> as Check>::describe(rest),
                "reportedAt" => <Option<String> as Check>::describe(rest),
                "dialled" => None,
                "blocked" => <Option<String> as Check>::describe(rest),
                "toolsBlocked" => <Option<String> as Check>::describe(rest),
                "build" => <Option<String> as Check>::describe(rest),
                "buildStopped" => <Option<bool> as Check>::describe(rest),
                "buildsImages" => <Option<bool> as Check>::describe(rest),
                "setup" => None,
                "applied" => None,
                "picks" => <Option<RecipeFile> as Check>::describe(rest),
                "recipe" => <Option<String> as Check>::describe(rest),
                "sync" => None,
                "cap" => <Option<PlaceCap> as Check>::describe(rest),
                "capDefault" => <Option<PlaceCap> as Check>::describe(rest),
                "settings" => <Option<PlaceSettings> as Check>::describe(rest),
                "napMs" => <Option<Option<i64>> as Check>::describe(rest),
                "napDefault" => <Option<i64> as Check>::describe(rest),
                "turnLimitMs" => <Option<Option<i64>> as Check>::describe(rest),
                "turnLimitDefault" => <Option<Option<i64>> as Check>::describe(rest),
                "spawn" => <Option<WorkspaceAgents> as Check>::describe(rest),
                "spawnDefault" => <Option<WorkspaceAgents> as Check>::describe(rest),
                "running" => <Option<i64> as Check>::describe(rest),
                "plan" => <Option<CloudPlan> as Check>::describe(rest),
                "starts" => <Option<CloudStarts> as Check>::describe(rest),
                "size" => <Option<PlaceViewSize> as Check>::describe(rest),
                "sizeDefault" => <Option<PlaceViewSizeDefault> as Check>::describe(rest),
                "deleteAfterMs" => <Option<i64> as Check>::describe(rest),
                "creditHours" => <Option<JsNumber> as Check>::describe(rest),
                "spentTodayUsd" => <Option<JsNumber> as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

/// TS: packages/protocol/src/views/place.ts:662 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaceDial {
    pub dialled: PlaceDialled,
    pub line: String,
    pub place: PlaceView,
}

impl Check for PlaceDial {
    /// Nested checks: dialled, place.
    fn check(&self) -> Result<(), SchemaError> {
        self.place.check().map_err(|e| e.under("place"))?;
        Ok(())
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "dialled" => None,
                "line" => <String as Check>::describe(rest),
                "place" => <PlaceView as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PlaceUpdateReplyDaemonRoad {
    #[serde(rename = "link")]
    Link,
    #[serde(rename = "ssh")]
    Ssh,
}

impl Check for PlaceUpdateReplyDaemonRoad {
    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        crate::types::check::leaf(path, crate::types::check::Zod::Options(&["link", "ssh"]))
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaceUpdateReplyDaemon {
    pub from: i64,
    pub to: i64,
    pub road: PlaceUpdateReplyDaemonRoad,
    pub at: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kept: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

impl Check for PlaceUpdateReplyDaemon {
    fn check(&self) -> Result<(), SchemaError> {
        self.road.check().map_err(|e| e.under("road"))?;
        Ok(())
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "from" => <i64 as Check>::describe(rest),
                "to" => <i64 as Check>::describe(rest),
                "road" => <PlaceUpdateReplyDaemonRoad as Check>::describe(rest),
                "at" => <String as Check>::describe(rest),
                "kept" => <Option<String> as Check>::describe(rest),
                "note" => <Option<String> as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

/// TS: packages/protocol/src/views/place.ts:671 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaceUpdateReply {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub daemon: Option<PlaceUpdateReplyDaemon>,
}

impl Check for PlaceUpdateReply {
    /// Rules: name: at least 1 UTF-16 units.
    /// Nested checks: daemon.
    fn check(&self) -> Result<(), SchemaError> {
        crate::types::check::text_at_least(&self.name, 1).map_err(|e| e.under("name"))?;
        self.daemon.check().map_err(|e| e.under("daemon"))?;
        Ok(())
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "name" => <String as Check>::describe(rest),
                "daemon" => <Option<PlaceUpdateReplyDaemon> as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaceHoldsForksItem {
    pub name: String,
    pub threads: i64,
}

impl Check for PlaceHoldsForksItem {
    fn check(&self) -> Result<(), SchemaError> {
        Ok(())
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "name" => <String as Check>::describe(rest),
                "threads" => <i64 as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaceHoldsImagesItem {
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub size_bytes: Option<JsNumber>,
}

impl Check for PlaceHoldsImagesItem {
    /// Rules: sizeBytes: >= 0.
    fn check(&self) -> Result<(), SchemaError> {
        if let Some(v) = self.size_bytes {
            crate::types::check::num_at_least(v.0, 0.0).map_err(|e| e.under("sizeBytes"))?;
        }
        Ok(())
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "id" => <String as Check>::describe(rest),
                "sizeBytes" => <Option<JsNumber> as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

/// TS: packages/protocol/src/views/place.ts:691 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaceHolds {
    pub forks: Vec<PlaceHoldsForksItem>,
    pub projects: Vec<PlaceHoldsForksItem>,
    pub unsaved: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub away: Option<True>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub images: Option<Vec<PlaceHoldsImagesItem>>,
}

impl Check for PlaceHolds {
    /// Nested checks: forks, images.
    fn check(&self) -> Result<(), SchemaError> {
        self.forks.check().map_err(|e| e.under("forks"))?;
        self.projects.check().map_err(|e| e.under("projects"))?;
        self.images.check().map_err(|e| e.under("images"))?;
        Ok(())
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "forks" => <Vec<PlaceHoldsForksItem> as Check>::describe(rest),
                "projects" => <Vec<PlaceHoldsForksItem> as Check>::describe(rest),
                "unsaved" => <Vec<String> as Check>::describe(rest),
                "away" => None,
                "images" => <Option<Vec<PlaceHoldsImagesItem>> as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaceRemovedTook {
    pub forks: Vec<PlaceHoldsForksItem>,
    pub projects: Vec<PlaceHoldsForksItem>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub away: Option<True>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub images: Option<Vec<PlaceHoldsImagesItem>>,
}

impl Check for PlaceRemovedTook {
    fn check(&self) -> Result<(), SchemaError> {
        self.forks.check().map_err(|e| e.under("forks"))?;
        self.projects.check().map_err(|e| e.under("projects"))?;
        self.images.check().map_err(|e| e.under("images"))?;
        Ok(())
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "forks" => <Vec<PlaceHoldsForksItem> as Check>::describe(rest),
                "projects" => <Vec<PlaceHoldsForksItem> as Check>::describe(rest),
                "away" => None,
                "images" => <Option<Vec<PlaceHoldsImagesItem>> as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

/// TS: packages/protocol/src/views/place.ts:709 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaceRemoved {
    pub removed: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub took: Option<PlaceRemovedTook>,
    pub swept: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub deleted: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub snapshots: Option<i64>,
}

impl Check for PlaceRemoved {
    /// Nested checks: took.
    fn check(&self) -> Result<(), SchemaError> {
        self.took.check().map_err(|e| e.under("took"))?;
        Ok(())
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "removed" => <bool as Check>::describe(rest),
                "took" => <Option<PlaceRemovedTook> as Check>::describe(rest),
                "swept" => <Vec<String> as Check>::describe(rest),
                "note" => <Option<String> as Check>::describe(rest),
                "deleted" => <Option<i64> as Check>::describe(rest),
                "snapshots" => <Option<i64> as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

/// TS: packages/protocol/src/views/place.ts:717 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaceSpend {
    pub place: String,
    pub today_usd: JsNumber,
    pub month_usd: JsNumber,
    pub rate_usd_per_hour: JsNumber,
}

impl Check for PlaceSpend {
    fn check(&self) -> Result<(), SchemaError> {
        Ok(())
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "place" => <String as Check>::describe(rest),
                "todayUsd" => <JsNumber as Check>::describe(rest),
                "monthUsd" => <JsNumber as Check>::describe(rest),
                "rateUsdPerHour" => <JsNumber as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

/// TS: packages/protocol/src/views/place.ts:722 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaceJoinedEvent {
    pub place: PlaceView,
    pub from: String,
}

impl PlaceJoinedEvent {
    /// The `type` key's value: the enum holding this struct writes and reads it as its tag.
    pub const TYPE: &'static str = "place.joined";
}

impl Check for PlaceJoinedEvent {
    /// Nested checks: place.
    fn check(&self) -> Result<(), SchemaError> {
        self.place.check().map_err(|e| e.under("place"))?;
        Ok(())
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "place" => <PlaceView as Check>::describe(rest),
                "from" => <String as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

/// TS: packages/protocol/src/views/place.ts:723 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlacePresentEvent {
    pub place_id: String,
    pub from: String,
}

impl PlacePresentEvent {
    /// The `type` key's value: the enum holding this struct writes and reads it as its tag.
    pub const TYPE: &'static str = "place.present";
}

impl Check for PlacePresentEvent {
    fn check(&self) -> Result<(), SchemaError> {
        Ok(())
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "placeId" => <String as Check>::describe(rest),
                "from" => <String as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

/// TS: packages/protocol/src/views/place.ts:725 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaceAbsentEvent {
    pub place_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub said: Option<String>,
}

impl PlaceAbsentEvent {
    /// The `type` key's value: the enum holding this struct writes and reads it as its tag.
    pub const TYPE: &'static str = "place.absent";
}

impl Check for PlaceAbsentEvent {
    fn check(&self) -> Result<(), SchemaError> {
        Ok(())
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "placeId" => <String as Check>::describe(rest),
                "said" => <Option<String> as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

/// TS: packages/protocol/src/views/place.ts:726 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaceRemovedEvent {
    pub place_id: String,
}

impl PlaceRemovedEvent {
    /// The `type` key's value: the enum holding this struct writes and reads it as its tag.
    pub const TYPE: &'static str = "place.removed";
}

impl Check for PlaceRemovedEvent {
    fn check(&self) -> Result<(), SchemaError> {
        Ok(())
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "placeId" => <String as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

/// TS: packages/protocol/src/views/place.ts:728 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaceChangedEvent {
    pub place: PlaceView,
}

impl PlaceChangedEvent {
    /// The `type` key's value: the enum holding this struct writes and reads it as its tag.
    pub const TYPE: &'static str = "place.changed";
}

impl Check for PlaceChangedEvent {
    /// Nested checks: place.
    fn check(&self) -> Result<(), SchemaError> {
        self.place.check().map_err(|e| e.under("place"))?;
        Ok(())
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "place" => <PlaceView as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

/// TS: packages/protocol/src/views/place.ts:731 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlacePendingEvent {
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pending: Option<PendingComputer>,
}

impl PlacePendingEvent {
    /// The `type` key's value: the enum holding this struct writes and reads it as its tag.
    pub const TYPE: &'static str = "place.pending";
}

impl Check for PlacePendingEvent {
    /// Nested checks: pending.
    fn check(&self) -> Result<(), SchemaError> {
        Ok(())
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "id" => <String as Check>::describe(rest),
                "pending" => None,
                _ => None,
            },
        }
    }
}

/// TS: packages/protocol/src/views/place.ts:738 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectAddedEvent {
    pub project: ProjectView,
}

impl ProjectAddedEvent {
    /// The `type` key's value: the enum holding this struct writes and reads it as its tag.
    pub const TYPE: &'static str = "project.added";
}

impl Check for ProjectAddedEvent {
    /// Nested checks: project.
    fn check(&self) -> Result<(), SchemaError> {
        self.project.check().map_err(|e| e.under("project"))?;
        Ok(())
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "project" => <ProjectView as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

/// TS: packages/protocol/src/views/place.ts:743 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectAddEvent {
    pub project_id: String,
    pub computer: String,
    pub stage: ProjectAddStage,
    pub message: String,
    pub elapsed_ms: JsNumber,
}

impl ProjectAddEvent {
    /// The `type` key's value: the enum holding this struct writes and reads it as its tag.
    pub const TYPE: &'static str = "project.add";
}

impl Check for ProjectAddEvent {
    /// Nested checks: stage.
    fn check(&self) -> Result<(), SchemaError> {
        self.stage.check().map_err(|e| e.under("stage"))?;
        Ok(())
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "projectId" => <String as Check>::describe(rest),
                "computer" => <String as Check>::describe(rest),
                "stage" => <ProjectAddStage as Check>::describe(rest),
                "message" => <String as Check>::describe(rest),
                "elapsedMs" => <JsNumber as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

/// TS: packages/protocol/src/views/place.ts:754 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HostNoticeEvent {
    pub message: String,
}

impl HostNoticeEvent {
    /// The `type` key's value: the enum holding this struct writes and reads it as its tag.
    pub const TYPE: &'static str = "host.notice";
}

impl Check for HostNoticeEvent {
    fn check(&self) -> Result<(), SchemaError> {
        Ok(())
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "message" => <String as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

/// TS: packages/protocol/src/views/place.ts:757 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AsideTextEvent {
    pub workspace_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub thread_id: Option<String>,
    pub ask_id: String,
    pub text: String,
}

impl AsideTextEvent {
    /// The `type` key's value: the enum holding this struct writes and reads it as its tag.
    pub const TYPE: &'static str = "aside.text";
}

impl Check for AsideTextEvent {
    fn check(&self) -> Result<(), SchemaError> {
        Ok(())
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "workspaceId" => <String as Check>::describe(rest),
                "threadId" => <Option<String> as Check>::describe(rest),
                "askId" => <String as Check>::describe(rest),
                "text" => <String as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

/// TS: packages/protocol/src/views/place.ts:761 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectRemovedEvent {
    pub project_id: String,
}

impl ProjectRemovedEvent {
    /// The `type` key's value: the enum holding this struct writes and reads it as its tag.
    pub const TYPE: &'static str = "project.removed";
}

impl Check for ProjectRemovedEvent {
    fn check(&self) -> Result<(), SchemaError> {
        Ok(())
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "projectId" => <String as Check>::describe(rest),
                _ => None,
            },
        }
    }
}
