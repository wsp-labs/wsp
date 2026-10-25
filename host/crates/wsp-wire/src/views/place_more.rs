// SPDX-License-Identifier: AGPL-3.0-only
//! The later shapes of packages/protocol/src/views/place.ts at a3b870a6e, apart only for the repo's file-size law; place.rs re-exports them.

#![allow(unused_imports)]

use serde::{Deserialize, Serialize};

use super::place::*;
use crate::types::agents_report::AgentSignInState;
use crate::types::check::{Check, SchemaError};
use crate::types::js_number::JsInt;
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
    pub threads: JsInt,
}

impl Check for ComputerCap {
    /// Rules: threads: >= 1.
    fn check(&self) -> Result<(), SchemaError> {
        crate::types::check::num_at_least(self.threads.0.0, 1.0).map_err(|e| e.under("threads"))?;
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        match obj.get("threads") { None => out.push(crate::types::check::missing::<Self>("threads")), Some(v) => { match crate::types::check::read::<JsInt>(v) { Ok(x) => { if let Err(e) = Self::rule_threads(&x) { out.push(e.under("threads")); } }, Err(es) => out.extend(es.into_iter().map(|e| e.under("threads"))), } } }
        Some(out)
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
impl ComputerCap {
    fn rule_threads(x: &JsInt) -> Result<(), crate::types::check::SchemaError> {
        crate::types::check::num_at_least((*x).0.0, 1.0)?;
        Ok(())
    }
}

/// TS: packages/protocol/src/views/place.ts:435 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CloudCap {
    pub machines: JsInt,
    pub spend_per_day_usd: JsNumber,
}

impl Check for CloudCap {
    /// Rules: machines: >= 1; spendPerDayUsd: >= 0.
    fn check(&self) -> Result<(), SchemaError> {
        crate::types::check::num_at_least(self.machines.0.0, 1.0).map_err(|e| e.under("machines"))?;
        crate::types::check::num_at_least(self.spend_per_day_usd.0, 0.0).map_err(|e| e.under("spendPerDayUsd"))?;
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        match obj.get("machines") { None => out.push(crate::types::check::missing::<Self>("machines")), Some(v) => { match crate::types::check::read::<JsInt>(v) { Ok(x) => { if let Err(e) = Self::rule_machines(&x) { out.push(e.under("machines")); } }, Err(es) => out.extend(es.into_iter().map(|e| e.under("machines"))), } } }
        match obj.get("spendPerDayUsd") { None => out.push(crate::types::check::missing::<Self>("spendPerDayUsd")), Some(v) => { match crate::types::check::read::<JsNumber>(v) { Ok(x) => { if let Err(e) = Self::rule_spend_per_day_usd(&x) { out.push(e.under("spendPerDayUsd")); } }, Err(es) => out.extend(es.into_iter().map(|e| e.under("spendPerDayUsd"))), } } }
        Some(out)
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
impl CloudCap {
    fn rule_machines(x: &JsInt) -> Result<(), crate::types::check::SchemaError> {
        crate::types::check::num_at_least((*x).0.0, 1.0)?;
        Ok(())
    }
    fn rule_spend_per_day_usd(x: &JsNumber) -> Result<(), crate::types::check::SchemaError> {
        crate::types::check::num_at_least((*x).0, 0.0)?;
        Ok(())
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

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        Some(crate::types::check::union_answer(vec![crate::types::check::issues_of::<ComputerCap>(value), crate::types::check::issues_of::<CloudCap>(value)]))
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
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub threads: Option<JsInt>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub machines: Option<JsInt>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub spend_per_day_usd: Option<JsNumber>,
}

impl Check for PlaceCapSet {
    /// Rules: threads: >= 1; machines: >= 1; spendPerDayUsd: >= 0.
    fn check(&self) -> Result<(), SchemaError> {
        if let Some(v) = self.threads {
            crate::types::check::num_at_least(v.0.0, 1.0).map_err(|e| e.under("threads"))?;
        }
        if let Some(v) = self.machines {
            crate::types::check::num_at_least(v.0.0, 1.0).map_err(|e| e.under("machines"))?;
        }
        if let Some(v) = self.spend_per_day_usd {
            crate::types::check::num_at_least(v.0, 0.0).map_err(|e| e.under("spendPerDayUsd"))?;
        }
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        if let Some(v) = obj.get("threads") { match crate::types::check::read_present::<JsInt>(v) { Ok(x) => { if let Err(e) = Self::rule_threads(&x) { out.push(e.under("threads")); } }, Err(es) => out.extend(es.into_iter().map(|e| e.under("threads"))), } }
        if let Some(v) = obj.get("machines") { match crate::types::check::read_present::<JsInt>(v) { Ok(x) => { if let Err(e) = Self::rule_machines(&x) { out.push(e.under("machines")); } }, Err(es) => out.extend(es.into_iter().map(|e| e.under("machines"))), } }
        if let Some(v) = obj.get("spendPerDayUsd") { match crate::types::check::read_present::<JsNumber>(v) { Ok(x) => { if let Err(e) = Self::rule_spend_per_day_usd(&x) { out.push(e.under("spendPerDayUsd")); } }, Err(es) => out.extend(es.into_iter().map(|e| e.under("spendPerDayUsd"))), } }
        Some(out)
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
impl PlaceCapSet {
    fn rule_threads(x: &Option<JsInt>) -> Result<(), crate::types::check::SchemaError> {
        if let Some(v) = (*x) {
            crate::types::check::num_at_least(v.0.0, 1.0)?;
        }
        Ok(())
    }
    fn rule_machines(x: &Option<JsInt>) -> Result<(), crate::types::check::SchemaError> {
        if let Some(v) = (*x) {
            crate::types::check::num_at_least(v.0.0, 1.0)?;
        }
        Ok(())
    }
    fn rule_spend_per_day_usd(x: &Option<JsNumber>) -> Result<(), crate::types::check::SchemaError> {
        if let Some(v) = (*x) {
            crate::types::check::num_at_least(v.0, 0.0)?;
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaceSettingsSpawn {
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub spawn: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub max_machines: Option<JsInt>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub max_depth: Option<JsInt>,
}

impl Check for PlaceSettingsSpawn {
    /// Rules: maxMachines: >= 0; maxDepth: >= 1.
    fn check(&self) -> Result<(), SchemaError> {
        if let Some(v) = self.max_machines {
            crate::types::check::num_at_least(v.0.0, 0.0).map_err(|e| e.under("maxMachines"))?;
        }
        if let Some(v) = self.max_depth {
            crate::types::check::num_at_least(v.0.0, 1.0).map_err(|e| e.under("maxDepth"))?;
        }
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        if let Some(v) = obj.get("spawn") { match crate::types::check::read_present::<bool>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("spawn"))), } }
        if let Some(v) = obj.get("maxMachines") { match crate::types::check::read_present::<JsInt>(v) { Ok(x) => { if let Err(e) = Self::rule_max_machines(&x) { out.push(e.under("maxMachines")); } }, Err(es) => out.extend(es.into_iter().map(|e| e.under("maxMachines"))), } }
        if let Some(v) = obj.get("maxDepth") { match crate::types::check::read_present::<JsInt>(v) { Ok(x) => { if let Err(e) = Self::rule_max_depth(&x) { out.push(e.under("maxDepth")); } }, Err(es) => out.extend(es.into_iter().map(|e| e.under("maxDepth"))), } }
        Some(out)
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
impl PlaceSettingsSpawn {
    fn rule_max_machines(x: &Option<JsInt>) -> Result<(), crate::types::check::SchemaError> {
        if let Some(v) = (*x) {
            crate::types::check::num_at_least(v.0.0, 0.0)?;
        }
        Ok(())
    }
    fn rule_max_depth(x: &Option<JsInt>) -> Result<(), crate::types::check::SchemaError> {
        if let Some(v) = (*x) {
            crate::types::check::num_at_least(v.0.0, 1.0)?;
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaceSettingsSize {
    pub cpu: JsInt,
    pub mem_mb: JsInt,
}

impl Check for PlaceSettingsSize {
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

/// TS: packages/protocol/src/views/place.ts:444 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaceSettings {
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub threads: Option<JsInt>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub machines: Option<JsInt>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub spend_per_day_usd: Option<JsNumber>,
    #[serde(default, skip_serializing_if = "Option::is_none", with = "crate::types::check::double_option")]
    pub nap_ms: Option<Option<JsInt>>,
    #[serde(default, skip_serializing_if = "Option::is_none", with = "crate::types::check::double_option")]
    pub turn_limit_ms: Option<Option<JsInt>>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub spawn: Option<PlaceSettingsSpawn>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub size: Option<PlaceSettingsSize>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub delete_after_ms: Option<JsInt>,
}

impl Check for PlaceSettings {
    /// Rules: threads: >= 1; machines: >= 1; spendPerDayUsd: >= 0; napMs: >= 60000; napMs: <= 10800000; turnLimitMs: >= 3600000; turnLimitMs: <= 86400000; deleteAfterMs: >= 0; deleteAfterMs: <= 86400000.
    /// Nested checks: spawn, size.
    fn check(&self) -> Result<(), SchemaError> {
        if let Some(v) = self.threads {
            crate::types::check::num_at_least(v.0.0, 1.0).map_err(|e| e.under("threads"))?;
        }
        if let Some(v) = self.machines {
            crate::types::check::num_at_least(v.0.0, 1.0).map_err(|e| e.under("machines"))?;
        }
        if let Some(v) = self.spend_per_day_usd {
            crate::types::check::num_at_least(v.0, 0.0).map_err(|e| e.under("spendPerDayUsd"))?;
        }
        if let Some(Some(v)) = self.nap_ms {
            crate::types::check::num_at_least(v.0.0, 60000.0).map_err(|e| e.under("napMs"))?;
        }
        if let Some(Some(v)) = self.nap_ms {
            crate::types::check::num_at_most(v.0.0, 10800000.0).map_err(|e| e.under("napMs"))?;
        }
        if let Some(Some(v)) = self.turn_limit_ms {
            crate::types::check::num_at_least(v.0.0, 3600000.0).map_err(|e| e.under("turnLimitMs"))?;
        }
        if let Some(Some(v)) = self.turn_limit_ms {
            crate::types::check::num_at_most(v.0.0, 86400000.0).map_err(|e| e.under("turnLimitMs"))?;
        }
        self.spawn.check().map_err(|e| e.under("spawn"))?;
        self.size.check().map_err(|e| e.under("size"))?;
        if let Some(v) = self.delete_after_ms {
            crate::types::check::num_at_least(v.0.0, 0.0).map_err(|e| e.under("deleteAfterMs"))?;
        }
        if let Some(v) = self.delete_after_ms {
            crate::types::check::num_at_most(v.0.0, 86400000.0).map_err(|e| e.under("deleteAfterMs"))?;
        }
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        if let Some(v) = obj.get("threads") { match crate::types::check::read_present::<JsInt>(v) { Ok(x) => { if let Err(e) = Self::rule_threads(&x) { out.push(e.under("threads")); } }, Err(es) => out.extend(es.into_iter().map(|e| e.under("threads"))), } }
        if let Some(v) = obj.get("machines") { match crate::types::check::read_present::<JsInt>(v) { Ok(x) => { if let Err(e) = Self::rule_machines(&x) { out.push(e.under("machines")); } }, Err(es) => out.extend(es.into_iter().map(|e| e.under("machines"))), } }
        if let Some(v) = obj.get("spendPerDayUsd") { match crate::types::check::read_present::<JsNumber>(v) { Ok(x) => { if let Err(e) = Self::rule_spend_per_day_usd(&x) { out.push(e.under("spendPerDayUsd")); } }, Err(es) => out.extend(es.into_iter().map(|e| e.under("spendPerDayUsd"))), } }
        if let Some(v) = obj.get("napMs") { match crate::types::check::read_double::<JsInt>(v) { Ok(x) => { if let Err(e) = Self::rule_nap_ms(&x) { out.push(e.under("napMs")); } }, Err(es) => out.extend(es.into_iter().map(|e| e.under("napMs"))), } }
        if let Some(v) = obj.get("turnLimitMs") { match crate::types::check::read_double::<JsInt>(v) { Ok(x) => { if let Err(e) = Self::rule_turn_limit_ms(&x) { out.push(e.under("turnLimitMs")); } }, Err(es) => out.extend(es.into_iter().map(|e| e.under("turnLimitMs"))), } }
        if let Some(v) = obj.get("spawn") { match crate::types::check::read_present::<PlaceSettingsSpawn>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("spawn"))), } }
        if let Some(v) = obj.get("size") { match crate::types::check::read_present::<PlaceSettingsSize>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("size"))), } }
        if let Some(v) = obj.get("deleteAfterMs") { match crate::types::check::read_present::<JsInt>(v) { Ok(x) => { if let Err(e) = Self::rule_delete_after_ms(&x) { out.push(e.under("deleteAfterMs")); } }, Err(es) => out.extend(es.into_iter().map(|e| e.under("deleteAfterMs"))), } }
        Some(out)
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
impl PlaceSettings {
    fn rule_threads(x: &Option<JsInt>) -> Result<(), crate::types::check::SchemaError> {
        if let Some(v) = (*x) {
            crate::types::check::num_at_least(v.0.0, 1.0)?;
        }
        Ok(())
    }
    fn rule_machines(x: &Option<JsInt>) -> Result<(), crate::types::check::SchemaError> {
        if let Some(v) = (*x) {
            crate::types::check::num_at_least(v.0.0, 1.0)?;
        }
        Ok(())
    }
    fn rule_spend_per_day_usd(x: &Option<JsNumber>) -> Result<(), crate::types::check::SchemaError> {
        if let Some(v) = (*x) {
            crate::types::check::num_at_least(v.0, 0.0)?;
        }
        Ok(())
    }
    fn rule_nap_ms(x: &Option<Option<JsInt>>) -> Result<(), crate::types::check::SchemaError> {
        if let Some(Some(v)) = (*x) {
            crate::types::check::num_at_least(v.0.0, 60000.0)?;
        }
        if let Some(Some(v)) = (*x) {
            crate::types::check::num_at_most(v.0.0, 10800000.0)?;
        }
        Ok(())
    }
    fn rule_turn_limit_ms(x: &Option<Option<JsInt>>) -> Result<(), crate::types::check::SchemaError> {
        if let Some(Some(v)) = (*x) {
            crate::types::check::num_at_least(v.0.0, 3600000.0)?;
        }
        if let Some(Some(v)) = (*x) {
            crate::types::check::num_at_most(v.0.0, 86400000.0)?;
        }
        Ok(())
    }
    fn rule_delete_after_ms(x: &Option<JsInt>) -> Result<(), crate::types::check::SchemaError> {
        if let Some(v) = (*x) {
            crate::types::check::num_at_least(v.0.0, 0.0)?;
        }
        if let Some(v) = (*x) {
            crate::types::check::num_at_most(v.0.0, 86400000.0)?;
        }
        Ok(())
    }
}

/// TS: packages/protocol/src/views/place.ts:458 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaceSettingsAsk {
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub threads: Option<JsInt>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub machines: Option<JsInt>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub spend_per_day_usd: Option<JsNumber>,
    #[serde(default, skip_serializing_if = "Option::is_none", with = "crate::types::check::double_option")]
    pub nap_ms: Option<Option<JsInt>>,
    #[serde(default, skip_serializing_if = "Option::is_none", with = "crate::types::check::double_option")]
    pub turn_limit_ms: Option<Option<JsInt>>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub spawn: Option<PlaceSettingsSpawn>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub size: Option<PlaceSettingsSize>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub delete_after_ms: Option<JsInt>,
}

impl Check for PlaceSettingsAsk {
    /// Rules: threads: >= 1; machines: >= 1; spendPerDayUsd: >= 0; napMs: >= 60000; napMs: <= 10800000; turnLimitMs: >= 3600000; turnLimitMs: <= 86400000; deleteAfterMs: >= 0; deleteAfterMs: <= 86400000.
    fn check(&self) -> Result<(), SchemaError> {
        if let Some(v) = self.threads {
            crate::types::check::num_at_least(v.0.0, 1.0).map_err(|e| e.under("threads"))?;
        }
        if let Some(v) = self.machines {
            crate::types::check::num_at_least(v.0.0, 1.0).map_err(|e| e.under("machines"))?;
        }
        if let Some(v) = self.spend_per_day_usd {
            crate::types::check::num_at_least(v.0, 0.0).map_err(|e| e.under("spendPerDayUsd"))?;
        }
        if let Some(Some(v)) = self.nap_ms {
            crate::types::check::num_at_least(v.0.0, 60000.0).map_err(|e| e.under("napMs"))?;
        }
        if let Some(Some(v)) = self.nap_ms {
            crate::types::check::num_at_most(v.0.0, 10800000.0).map_err(|e| e.under("napMs"))?;
        }
        if let Some(Some(v)) = self.turn_limit_ms {
            crate::types::check::num_at_least(v.0.0, 3600000.0).map_err(|e| e.under("turnLimitMs"))?;
        }
        if let Some(Some(v)) = self.turn_limit_ms {
            crate::types::check::num_at_most(v.0.0, 86400000.0).map_err(|e| e.under("turnLimitMs"))?;
        }
        self.spawn.check().map_err(|e| e.under("spawn"))?;
        self.size.check().map_err(|e| e.under("size"))?;
        if let Some(v) = self.delete_after_ms {
            crate::types::check::num_at_least(v.0.0, 0.0).map_err(|e| e.under("deleteAfterMs"))?;
        }
        if let Some(v) = self.delete_after_ms {
            crate::types::check::num_at_most(v.0.0, 86400000.0).map_err(|e| e.under("deleteAfterMs"))?;
        }
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        if let Some(v) = obj.get("threads") { match crate::types::check::read_present::<JsInt>(v) { Ok(x) => { if let Err(e) = Self::rule_threads(&x) { out.push(e.under("threads")); } }, Err(es) => out.extend(es.into_iter().map(|e| e.under("threads"))), } }
        if let Some(v) = obj.get("machines") { match crate::types::check::read_present::<JsInt>(v) { Ok(x) => { if let Err(e) = Self::rule_machines(&x) { out.push(e.under("machines")); } }, Err(es) => out.extend(es.into_iter().map(|e| e.under("machines"))), } }
        if let Some(v) = obj.get("spendPerDayUsd") { match crate::types::check::read_present::<JsNumber>(v) { Ok(x) => { if let Err(e) = Self::rule_spend_per_day_usd(&x) { out.push(e.under("spendPerDayUsd")); } }, Err(es) => out.extend(es.into_iter().map(|e| e.under("spendPerDayUsd"))), } }
        if let Some(v) = obj.get("napMs") { match crate::types::check::read_double::<JsInt>(v) { Ok(x) => { if let Err(e) = Self::rule_nap_ms(&x) { out.push(e.under("napMs")); } }, Err(es) => out.extend(es.into_iter().map(|e| e.under("napMs"))), } }
        if let Some(v) = obj.get("turnLimitMs") { match crate::types::check::read_double::<JsInt>(v) { Ok(x) => { if let Err(e) = Self::rule_turn_limit_ms(&x) { out.push(e.under("turnLimitMs")); } }, Err(es) => out.extend(es.into_iter().map(|e| e.under("turnLimitMs"))), } }
        if let Some(v) = obj.get("spawn") { match crate::types::check::read_present::<PlaceSettingsSpawn>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("spawn"))), } }
        if let Some(v) = obj.get("size") { match crate::types::check::read_present::<PlaceSettingsSize>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("size"))), } }
        if let Some(v) = obj.get("deleteAfterMs") { match crate::types::check::read_present::<JsInt>(v) { Ok(x) => { if let Err(e) = Self::rule_delete_after_ms(&x) { out.push(e.under("deleteAfterMs")); } }, Err(es) => out.extend(es.into_iter().map(|e| e.under("deleteAfterMs"))), } }
        Some(out)
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
impl PlaceSettingsAsk {
    fn rule_threads(x: &Option<JsInt>) -> Result<(), crate::types::check::SchemaError> {
        if let Some(v) = (*x) {
            crate::types::check::num_at_least(v.0.0, 1.0)?;
        }
        Ok(())
    }
    fn rule_machines(x: &Option<JsInt>) -> Result<(), crate::types::check::SchemaError> {
        if let Some(v) = (*x) {
            crate::types::check::num_at_least(v.0.0, 1.0)?;
        }
        Ok(())
    }
    fn rule_spend_per_day_usd(x: &Option<JsNumber>) -> Result<(), crate::types::check::SchemaError> {
        if let Some(v) = (*x) {
            crate::types::check::num_at_least(v.0, 0.0)?;
        }
        Ok(())
    }
    fn rule_nap_ms(x: &Option<Option<JsInt>>) -> Result<(), crate::types::check::SchemaError> {
        if let Some(Some(v)) = (*x) {
            crate::types::check::num_at_least(v.0.0, 60000.0)?;
        }
        if let Some(Some(v)) = (*x) {
            crate::types::check::num_at_most(v.0.0, 10800000.0)?;
        }
        Ok(())
    }
    fn rule_turn_limit_ms(x: &Option<Option<JsInt>>) -> Result<(), crate::types::check::SchemaError> {
        if let Some(Some(v)) = (*x) {
            crate::types::check::num_at_least(v.0.0, 3600000.0)?;
        }
        if let Some(Some(v)) = (*x) {
            crate::types::check::num_at_most(v.0.0, 86400000.0)?;
        }
        Ok(())
    }
    fn rule_delete_after_ms(x: &Option<JsInt>) -> Result<(), crate::types::check::SchemaError> {
        if let Some(v) = (*x) {
            crate::types::check::num_at_least(v.0.0, 0.0)?;
        }
        if let Some(v) = (*x) {
            crate::types::check::num_at_most(v.0.0, 86400000.0)?;
        }
        Ok(())
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
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub key: Option<String>,
    pub trial: bool,
    pub machines_at_once: JsInt,
    pub sizes: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub running: Option<JsInt>,
}

impl Check for CloudPlan {
    fn check(&self) -> Result<(), SchemaError> {
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        if let Some(v) = obj.get("key") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("key"))), } }
        match obj.get("trial") { None => out.push(crate::types::check::missing::<Self>("trial")), Some(v) => { match crate::types::check::read::<bool>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("trial"))), } } }
        match obj.get("machinesAtOnce") { None => out.push(crate::types::check::missing::<Self>("machinesAtOnce")), Some(v) => { match crate::types::check::read::<JsInt>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("machinesAtOnce"))), } } }
        match obj.get("sizes") { None => out.push(crate::types::check::missing::<Self>("sizes")), Some(v) => { match crate::types::check::read::<Vec<String>>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("sizes"))), } } }
        if let Some(v) = obj.get("running") { match crate::types::check::read_present::<JsInt>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("running"))), } }
        Some(out)
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
    pub used: JsInt,
    pub limit: JsInt,
}

impl Check for StartsWindow {
    fn check(&self) -> Result<(), SchemaError> {
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        match obj.get("used") { None => out.push(crate::types::check::missing::<Self>("used")), Some(v) => { match crate::types::check::read::<JsInt>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("used"))), } } }
        match obj.get("limit") { None => out.push(crate::types::check::missing::<Self>("limit")), Some(v) => { match crate::types::check::read::<JsInt>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("limit"))), } } }
        Some(out)
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

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        match obj.get("minute") { None => out.push(crate::types::check::missing::<Self>("minute")), Some(v) => { match crate::types::check::read::<StartsWindow>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("minute"))), } } }
        match obj.get("hour") { None => out.push(crate::types::check::missing::<Self>("hour")), Some(v) => { match crate::types::check::read::<StartsWindow>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("hour"))), } } }
        match obj.get("day") { None => out.push(crate::types::check::missing::<Self>("day")), Some(v) => { match crate::types::check::read::<StartsWindow>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("day"))), } } }
        Some(out)
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
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub starts: Option<CloudStarts>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub credit_hours: Option<JsNumber>,
}

impl Check for CloudLimits {
    /// Nested checks: plan, starts.
    fn check(&self) -> Result<(), SchemaError> {
        self.plan.check().map_err(|e| e.under("plan"))?;
        self.starts.check().map_err(|e| e.under("starts"))?;
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        match obj.get("plan") { None => out.push(crate::types::check::missing::<Self>("plan")), Some(v) => { match crate::types::check::read::<CloudPlan>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("plan"))), } } }
        if let Some(v) = obj.get("starts") { match crate::types::check::read_present::<CloudStarts>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("starts"))), } }
        if let Some(v) = obj.get("creditHours") { match crate::types::check::read_present::<JsNumber>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("creditHours"))), } }
        Some(out)
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

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        match obj.get("word") { None => out.push(crate::types::check::missing::<Self>("word")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("word"))), } } }
        match obj.get("fix") { None => out.push(crate::types::check::missing::<Self>("fix")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("fix"))), } } }
        match obj.get("act") { None => out.push(crate::types::check::missing::<Self>("act")), Some(v) => { match crate::types::check::read::<PlaceViewBehindAct>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("act"))), } } }
        Some(out)
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
    pub running: JsInt,
    pub room: JsInt,
}

impl Check for PlaceViewForks {
    fn check(&self) -> Result<(), SchemaError> {
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        match obj.get("running") { None => out.push(crate::types::check::missing::<Self>("running")), Some(v) => { match crate::types::check::read::<JsInt>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("running"))), } } }
        match obj.get("room") { None => out.push(crate::types::check::missing::<Self>("room")), Some(v) => { match crate::types::check::read::<JsInt>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("room"))), } } }
        Some(out)
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
    pub cpu: JsInt,
    pub mem_mb: JsInt,
}

impl Check for PlaceViewSize {
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
pub struct PlaceViewSizeDefault {
    pub cpu: JsInt,
    pub mem_mb: JsInt,
}

impl Check for PlaceViewSizeDefault {
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

/// TS: packages/protocol/src/views/place.ts:513 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaceView {
    pub id: String,
    pub kind: PlaceKind,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub label: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub mac: Option<MacKind>,
    pub default: bool,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub os: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub shape: Option<WorkspaceSize>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub disk_free_bytes: Option<JsInt>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub engine: Option<PlaceViewEngine>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub copies: Option<CopyWord>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub present: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub takes_runtime: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub joined_at: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub last_seen_at: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub daemon_version: Option<JsInt>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub behind: Option<PlaceViewBehind>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub agents: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present", serialize_with = "crate::types::check::js_record_opt")]
    pub agent_versions: Option<IndexMap<String, String>>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present", serialize_with = "crate::types::check::js_record_opt")]
    pub sign_ins: Option<IndexMap<String, AgentSignInState>>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub logins: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub rate_usd_per_hour: Option<JsNumber>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub sizes: Option<Vec<MachineSizeOffer>>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub forks: Option<PlaceViewForks>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub takes_forks: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub road: Option<PlaceRoad>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub home: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub uptime_ms: Option<JsInt>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub reported_at: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub dialled: Option<PlaceDialled>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub blocked: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub tools_blocked: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub build: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub build_stopped: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub builds_images: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub setup: Option<PlaceSetup>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub applied: Option<PlaceApplied>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub picks: Option<RecipeFile>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub recipe: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub sync: Option<PlaceSync>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub cap: Option<PlaceCap>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub cap_default: Option<PlaceCap>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub settings: Option<PlaceSettings>,
    #[serde(default, skip_serializing_if = "Option::is_none", with = "crate::types::check::double_option")]
    pub nap_ms: Option<Option<JsInt>>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub nap_default: Option<JsInt>,
    #[serde(default, skip_serializing_if = "Option::is_none", with = "crate::types::check::double_option")]
    pub turn_limit_ms: Option<Option<JsInt>>,
    #[serde(default, skip_serializing_if = "Option::is_none", with = "crate::types::check::double_option")]
    pub turn_limit_default: Option<Option<JsInt>>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub spawn: Option<WorkspaceAgents>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub spawn_default: Option<WorkspaceAgents>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub running: Option<JsInt>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub plan: Option<CloudPlan>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub starts: Option<CloudStarts>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub size: Option<PlaceViewSize>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub size_default: Option<PlaceViewSizeDefault>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub delete_after_ms: Option<JsInt>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub credit_hours: Option<JsNumber>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
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
            crate::types::check::num_at_least(v.0.0, 0.0).map_err(|e| e.under("uptimeMs"))?;
        }
        self.picks.check().map_err(|e| e.under("picks"))?;
        self.cap.check().map_err(|e| e.under("cap"))?;
        self.cap_default.check().map_err(|e| e.under("capDefault"))?;
        self.settings.check().map_err(|e| e.under("settings"))?;
        self.spawn.check().map_err(|e| e.under("spawn"))?;
        self.spawn_default.check().map_err(|e| e.under("spawnDefault"))?;
        if let Some(v) = self.running {
            crate::types::check::num_at_least(v.0.0, 0.0).map_err(|e| e.under("running"))?;
        }
        self.plan.check().map_err(|e| e.under("plan"))?;
        self.starts.check().map_err(|e| e.under("starts"))?;
        self.size.check().map_err(|e| e.under("size"))?;
        self.size_default.check().map_err(|e| e.under("sizeDefault"))?;
        self.shape.check().map_err(|e| e.under("shape"))?;
        self.copies.check().map_err(|e| e.under("copies"))?;
        self.road.check().map_err(|e| e.under("road"))?;
        self.dialled.check().map_err(|e| e.under("dialled"))?;
        self.setup.check().map_err(|e| e.under("setup"))?;
        self.applied.check().map_err(|e| e.under("applied"))?;
        self.sync.check().map_err(|e| e.under("sync"))?;
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        match obj.get("id") { None => out.push(crate::types::check::missing::<Self>("id")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("id"))), } } }
        match obj.get("kind") { None => out.push(crate::types::check::missing::<Self>("kind")), Some(v) => { match crate::types::check::read::<PlaceKind>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("kind"))), } } }
        match obj.get("name") { None => out.push(crate::types::check::missing::<Self>("name")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("name"))), } } }
        if let Some(v) = obj.get("label") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("label"))), } }
        if let Some(v) = obj.get("mac") { match crate::types::check::read_present::<MacKind>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("mac"))), } }
        match obj.get("default") { None => out.push(crate::types::check::missing::<Self>("default")), Some(v) => { match crate::types::check::read::<bool>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("default"))), } } }
        if let Some(v) = obj.get("os") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("os"))), } }
        if let Some(v) = obj.get("shape") { match crate::types::check::read_present::<WorkspaceSize>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("shape"))), } }
        if let Some(v) = obj.get("diskFreeBytes") { match crate::types::check::read_present::<JsInt>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("diskFreeBytes"))), } }
        if let Some(v) = obj.get("engine") { match crate::types::check::read_present::<PlaceViewEngine>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("engine"))), } }
        if let Some(v) = obj.get("copies") { match crate::types::check::read_present::<CopyWord>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("copies"))), } }
        if let Some(v) = obj.get("present") { match crate::types::check::read_present::<bool>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("present"))), } }
        if let Some(v) = obj.get("takesRuntime") { match crate::types::check::read_present::<bool>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("takesRuntime"))), } }
        if let Some(v) = obj.get("joinedAt") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("joinedAt"))), } }
        if let Some(v) = obj.get("lastSeenAt") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("lastSeenAt"))), } }
        if let Some(v) = obj.get("daemonVersion") { match crate::types::check::read_present::<JsInt>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("daemonVersion"))), } }
        if let Some(v) = obj.get("behind") { match crate::types::check::read_present::<PlaceViewBehind>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("behind"))), } }
        if let Some(v) = obj.get("agents") { match crate::types::check::read_present::<Vec<String>>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("agents"))), } }
        if let Some(v) = obj.get("agentVersions") { match crate::types::check::read_present::<IndexMap<String, String>>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("agentVersions"))), } }
        if let Some(v) = obj.get("signIns") { match crate::types::check::read_present::<IndexMap<String, AgentSignInState>>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("signIns"))), } }
        if let Some(v) = obj.get("logins") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("logins"))), } }
        if let Some(v) = obj.get("rateUsdPerHour") { match crate::types::check::read_present::<JsNumber>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("rateUsdPerHour"))), } }
        if let Some(v) = obj.get("sizes") { match crate::types::check::read_present::<Vec<MachineSizeOffer>>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("sizes"))), } }
        if let Some(v) = obj.get("forks") { match crate::types::check::read_present::<PlaceViewForks>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("forks"))), } }
        if let Some(v) = obj.get("takesForks") { match crate::types::check::read_present::<bool>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("takesForks"))), } }
        if let Some(v) = obj.get("road") { match crate::types::check::read_present::<PlaceRoad>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("road"))), } }
        if let Some(v) = obj.get("home") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("home"))), } }
        if let Some(v) = obj.get("uptimeMs") { match crate::types::check::read_present::<JsInt>(v) { Ok(x) => { if let Err(e) = Self::rule_uptime_ms(&x) { out.push(e.under("uptimeMs")); } }, Err(es) => out.extend(es.into_iter().map(|e| e.under("uptimeMs"))), } }
        if let Some(v) = obj.get("reportedAt") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("reportedAt"))), } }
        if let Some(v) = obj.get("dialled") { match crate::types::check::read_present::<PlaceDialled>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("dialled"))), } }
        if let Some(v) = obj.get("blocked") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("blocked"))), } }
        if let Some(v) = obj.get("toolsBlocked") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("toolsBlocked"))), } }
        if let Some(v) = obj.get("build") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("build"))), } }
        if let Some(v) = obj.get("buildStopped") { match crate::types::check::read_present::<bool>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("buildStopped"))), } }
        if let Some(v) = obj.get("buildsImages") { match crate::types::check::read_present::<bool>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("buildsImages"))), } }
        if let Some(v) = obj.get("setup") { match crate::types::check::read_present::<PlaceSetup>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("setup"))), } }
        if let Some(v) = obj.get("applied") { match crate::types::check::read_present::<PlaceApplied>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("applied"))), } }
        if let Some(v) = obj.get("picks") { match crate::types::check::read_present::<RecipeFile>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("picks"))), } }
        if let Some(v) = obj.get("recipe") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("recipe"))), } }
        if let Some(v) = obj.get("sync") { match crate::types::check::read_present::<PlaceSync>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("sync"))), } }
        if let Some(v) = obj.get("cap") { match crate::types::check::read_present::<PlaceCap>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("cap"))), } }
        if let Some(v) = obj.get("capDefault") { match crate::types::check::read_present::<PlaceCap>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("capDefault"))), } }
        if let Some(v) = obj.get("settings") { match crate::types::check::read_present::<PlaceSettings>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("settings"))), } }
        if let Some(v) = obj.get("napMs") { match crate::types::check::read_double::<JsInt>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("napMs"))), } }
        if let Some(v) = obj.get("napDefault") { match crate::types::check::read_present::<JsInt>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("napDefault"))), } }
        if let Some(v) = obj.get("turnLimitMs") { match crate::types::check::read_double::<JsInt>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("turnLimitMs"))), } }
        if let Some(v) = obj.get("turnLimitDefault") { match crate::types::check::read_double::<JsInt>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("turnLimitDefault"))), } }
        if let Some(v) = obj.get("spawn") { match crate::types::check::read_present::<WorkspaceAgents>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("spawn"))), } }
        if let Some(v) = obj.get("spawnDefault") { match crate::types::check::read_present::<WorkspaceAgents>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("spawnDefault"))), } }
        if let Some(v) = obj.get("running") { match crate::types::check::read_present::<JsInt>(v) { Ok(x) => { if let Err(e) = Self::rule_running(&x) { out.push(e.under("running")); } }, Err(es) => out.extend(es.into_iter().map(|e| e.under("running"))), } }
        if let Some(v) = obj.get("plan") { match crate::types::check::read_present::<CloudPlan>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("plan"))), } }
        if let Some(v) = obj.get("starts") { match crate::types::check::read_present::<CloudStarts>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("starts"))), } }
        if let Some(v) = obj.get("size") { match crate::types::check::read_present::<PlaceViewSize>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("size"))), } }
        if let Some(v) = obj.get("sizeDefault") { match crate::types::check::read_present::<PlaceViewSizeDefault>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("sizeDefault"))), } }
        if let Some(v) = obj.get("deleteAfterMs") { match crate::types::check::read_present::<JsInt>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("deleteAfterMs"))), } }
        if let Some(v) = obj.get("creditHours") { match crate::types::check::read_present::<JsNumber>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("creditHours"))), } }
        if let Some(v) = obj.get("spentTodayUsd") { match crate::types::check::read_present::<JsNumber>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("spentTodayUsd"))), } }
        Some(out)
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
impl PlaceView {
    fn rule_uptime_ms(x: &Option<JsInt>) -> Result<(), crate::types::check::SchemaError> {
        if let Some(v) = (*x) {
            crate::types::check::num_at_least(v.0.0, 0.0)?;
        }
        Ok(())
    }
    fn rule_running(x: &Option<JsInt>) -> Result<(), crate::types::check::SchemaError> {
        if let Some(v) = (*x) {
            crate::types::check::num_at_least(v.0.0, 0.0)?;
        }
        Ok(())
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
        self.dialled.check().map_err(|e| e.under("dialled"))?;
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        match obj.get("dialled") { None => out.push(crate::types::check::missing::<Self>("dialled")), Some(v) => { match crate::types::check::read::<PlaceDialled>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("dialled"))), } } }
        match obj.get("line") { None => out.push(crate::types::check::missing::<Self>("line")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("line"))), } } }
        match obj.get("place") { None => out.push(crate::types::check::missing::<Self>("place")), Some(v) => { match crate::types::check::read::<PlaceView>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("place"))), } } }
        Some(out)
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
    pub from: JsInt,
    pub to: JsInt,
    pub road: PlaceUpdateReplyDaemonRoad,
    pub at: String,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub kept: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub note: Option<String>,
}

impl Check for PlaceUpdateReplyDaemon {
    fn check(&self) -> Result<(), SchemaError> {
        self.road.check().map_err(|e| e.under("road"))?;
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        match obj.get("from") { None => out.push(crate::types::check::missing::<Self>("from")), Some(v) => { match crate::types::check::read::<JsInt>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("from"))), } } }
        match obj.get("to") { None => out.push(crate::types::check::missing::<Self>("to")), Some(v) => { match crate::types::check::read::<JsInt>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("to"))), } } }
        match obj.get("road") { None => out.push(crate::types::check::missing::<Self>("road")), Some(v) => { match crate::types::check::read::<PlaceUpdateReplyDaemonRoad>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("road"))), } } }
        match obj.get("at") { None => out.push(crate::types::check::missing::<Self>("at")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("at"))), } } }
        if let Some(v) = obj.get("kept") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("kept"))), } }
        if let Some(v) = obj.get("note") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("note"))), } }
        Some(out)
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
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
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

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        match obj.get("name") { None => out.push(crate::types::check::missing::<Self>("name")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(x) => { if let Err(e) = Self::rule_name(&x) { out.push(e.under("name")); } }, Err(es) => out.extend(es.into_iter().map(|e| e.under("name"))), } } }
        if let Some(v) = obj.get("daemon") { match crate::types::check::read_present::<PlaceUpdateReplyDaemon>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("daemon"))), } }
        Some(out)
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
impl PlaceUpdateReply {
    fn rule_name(x: &String) -> Result<(), crate::types::check::SchemaError> {
        crate::types::check::text_at_least(x, 1)?;
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaceHoldsForksItem {
    pub name: String,
    pub threads: JsInt,
}

impl Check for PlaceHoldsForksItem {
    fn check(&self) -> Result<(), SchemaError> {
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        match obj.get("name") { None => out.push(crate::types::check::missing::<Self>("name")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("name"))), } } }
        match obj.get("threads") { None => out.push(crate::types::check::missing::<Self>("threads")), Some(v) => { match crate::types::check::read::<JsInt>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("threads"))), } } }
        Some(out)
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
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
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

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        match obj.get("id") { None => out.push(crate::types::check::missing::<Self>("id")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("id"))), } } }
        if let Some(v) = obj.get("sizeBytes") { match crate::types::check::read_present::<JsNumber>(v) { Ok(x) => { if let Err(e) = Self::rule_size_bytes(&x) { out.push(e.under("sizeBytes")); } }, Err(es) => out.extend(es.into_iter().map(|e| e.under("sizeBytes"))), } }
        Some(out)
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
impl PlaceHoldsImagesItem {
    fn rule_size_bytes(x: &Option<JsNumber>) -> Result<(), crate::types::check::SchemaError> {
        if let Some(v) = (*x) {
            crate::types::check::num_at_least(v.0, 0.0)?;
        }
        Ok(())
    }
}

/// TS: packages/protocol/src/views/place.ts:691 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaceHolds {
    pub forks: Vec<PlaceHoldsForksItem>,
    pub projects: Vec<PlaceHoldsForksItem>,
    pub unsaved: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub away: Option<True>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
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

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        match obj.get("forks") { None => out.push(crate::types::check::missing::<Self>("forks")), Some(v) => { match crate::types::check::read::<Vec<PlaceHoldsForksItem>>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("forks"))), } } }
        match obj.get("projects") { None => out.push(crate::types::check::missing::<Self>("projects")), Some(v) => { match crate::types::check::read::<Vec<PlaceHoldsForksItem>>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("projects"))), } } }
        match obj.get("unsaved") { None => out.push(crate::types::check::missing::<Self>("unsaved")), Some(v) => { match crate::types::check::read::<Vec<String>>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("unsaved"))), } } }
        if let Some(v) = obj.get("away") { match crate::types::check::read_present::<True>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("away"))), } }
        if let Some(v) = obj.get("images") { match crate::types::check::read_present::<Vec<PlaceHoldsImagesItem>>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("images"))), } }
        Some(out)
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
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub away: Option<True>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub images: Option<Vec<PlaceHoldsImagesItem>>,
}

impl Check for PlaceRemovedTook {
    fn check(&self) -> Result<(), SchemaError> {
        self.forks.check().map_err(|e| e.under("forks"))?;
        self.projects.check().map_err(|e| e.under("projects"))?;
        self.images.check().map_err(|e| e.under("images"))?;
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        match obj.get("forks") { None => out.push(crate::types::check::missing::<Self>("forks")), Some(v) => { match crate::types::check::read::<Vec<PlaceHoldsForksItem>>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("forks"))), } } }
        match obj.get("projects") { None => out.push(crate::types::check::missing::<Self>("projects")), Some(v) => { match crate::types::check::read::<Vec<PlaceHoldsForksItem>>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("projects"))), } } }
        if let Some(v) = obj.get("away") { match crate::types::check::read_present::<True>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("away"))), } }
        if let Some(v) = obj.get("images") { match crate::types::check::read_present::<Vec<PlaceHoldsImagesItem>>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("images"))), } }
        Some(out)
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
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub took: Option<PlaceRemovedTook>,
    pub swept: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub note: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub deleted: Option<JsInt>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub snapshots: Option<JsInt>,
}

impl Check for PlaceRemoved {
    /// Nested checks: took.
    fn check(&self) -> Result<(), SchemaError> {
        self.took.check().map_err(|e| e.under("took"))?;
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        match obj.get("removed") { None => out.push(crate::types::check::missing::<Self>("removed")), Some(v) => { match crate::types::check::read::<bool>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("removed"))), } } }
        if let Some(v) = obj.get("took") { match crate::types::check::read_present::<PlaceRemovedTook>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("took"))), } }
        match obj.get("swept") { None => out.push(crate::types::check::missing::<Self>("swept")), Some(v) => { match crate::types::check::read::<Vec<String>>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("swept"))), } } }
        if let Some(v) = obj.get("note") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("note"))), } }
        if let Some(v) = obj.get("deleted") { match crate::types::check::read_present::<JsInt>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("deleted"))), } }
        if let Some(v) = obj.get("snapshots") { match crate::types::check::read_present::<JsInt>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("snapshots"))), } }
        Some(out)
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

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        match obj.get("place") { None => out.push(crate::types::check::missing::<Self>("place")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("place"))), } } }
        match obj.get("todayUsd") { None => out.push(crate::types::check::missing::<Self>("todayUsd")), Some(v) => { match crate::types::check::read::<JsNumber>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("todayUsd"))), } } }
        match obj.get("monthUsd") { None => out.push(crate::types::check::missing::<Self>("monthUsd")), Some(v) => { match crate::types::check::read::<JsNumber>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("monthUsd"))), } } }
        match obj.get("rateUsdPerHour") { None => out.push(crate::types::check::missing::<Self>("rateUsdPerHour")), Some(v) => { match crate::types::check::read::<JsNumber>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("rateUsdPerHour"))), } } }
        Some(out)
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

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        match obj.get("place") { None => out.push(crate::types::check::missing::<Self>("place")), Some(v) => { match crate::types::check::read::<PlaceView>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("place"))), } } }
        match obj.get("from") { None => out.push(crate::types::check::missing::<Self>("from")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("from"))), } } }
        Some(out)
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

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        match obj.get("placeId") { None => out.push(crate::types::check::missing::<Self>("placeId")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("placeId"))), } } }
        match obj.get("from") { None => out.push(crate::types::check::missing::<Self>("from")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("from"))), } } }
        Some(out)
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
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
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

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        match obj.get("placeId") { None => out.push(crate::types::check::missing::<Self>("placeId")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("placeId"))), } } }
        if let Some(v) = obj.get("said") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("said"))), } }
        Some(out)
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

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        match obj.get("placeId") { None => out.push(crate::types::check::missing::<Self>("placeId")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("placeId"))), } } }
        Some(out)
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

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        match obj.get("place") { None => out.push(crate::types::check::missing::<Self>("place")), Some(v) => { match crate::types::check::read::<PlaceView>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("place"))), } } }
        Some(out)
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
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub pending: Option<PendingComputer>,
}

impl PlacePendingEvent {
    /// The `type` key's value: the enum holding this struct writes and reads it as its tag.
    pub const TYPE: &'static str = "place.pending";
}

impl Check for PlacePendingEvent {
    /// Nested checks: pending.
    fn check(&self) -> Result<(), SchemaError> {
        self.pending.check().map_err(|e| e.under("pending"))?;
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        match obj.get("id") { None => out.push(crate::types::check::missing::<Self>("id")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("id"))), } } }
        if let Some(v) = obj.get("pending") { match crate::types::check::read_present::<PendingComputer>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("pending"))), } }
        Some(out)
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

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        match obj.get("project") { None => out.push(crate::types::check::missing::<Self>("project")), Some(v) => { match crate::types::check::read::<ProjectView>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("project"))), } } }
        Some(out)
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

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        match obj.get("projectId") { None => out.push(crate::types::check::missing::<Self>("projectId")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("projectId"))), } } }
        match obj.get("computer") { None => out.push(crate::types::check::missing::<Self>("computer")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("computer"))), } } }
        match obj.get("stage") { None => out.push(crate::types::check::missing::<Self>("stage")), Some(v) => { match crate::types::check::read::<ProjectAddStage>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("stage"))), } } }
        match obj.get("message") { None => out.push(crate::types::check::missing::<Self>("message")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("message"))), } } }
        match obj.get("elapsedMs") { None => out.push(crate::types::check::missing::<Self>("elapsedMs")), Some(v) => { match crate::types::check::read::<JsNumber>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("elapsedMs"))), } } }
        Some(out)
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

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        match obj.get("message") { None => out.push(crate::types::check::missing::<Self>("message")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("message"))), } } }
        Some(out)
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
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
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

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        match obj.get("workspaceId") { None => out.push(crate::types::check::missing::<Self>("workspaceId")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("workspaceId"))), } } }
        if let Some(v) = obj.get("threadId") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("threadId"))), } }
        match obj.get("askId") { None => out.push(crate::types::check::missing::<Self>("askId")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("askId"))), } } }
        match obj.get("text") { None => out.push(crate::types::check::missing::<Self>("text")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("text"))), } } }
        Some(out)
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

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        match obj.get("projectId") { None => out.push(crate::types::check::missing::<Self>("projectId")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("projectId"))), } } }
        Some(out)
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
