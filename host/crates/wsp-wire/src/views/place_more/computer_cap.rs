// SPDX-License-Identifier: AGPL-3.0-only
use super::*;

/// TS: packages/protocol/src/views/place.ts:432 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ComputerCap {
    pub threads: JsInt,
}
impl Check for ComputerCap {
    /// Rules: threads: >= 1.
    fn check(&self) -> Result<(), SchemaError> {
        crate::types::check::num_at_least(self.threads.0 .0, 1.0).map_err(|e| e.under("threads"))?;
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        crate::types::keys::req_key_with::<JsInt>(obj, "threads", &mut out, Self::rule_threads);
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
        crate::types::check::num_at_least(x.0 .0, 1.0)?;
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
        crate::types::check::num_at_least(self.machines.0 .0, 1.0).map_err(|e| e.under("machines"))?;
        crate::types::check::num_at_least(self.spend_per_day_usd.0, 0.0).map_err(|e| e.under("spendPerDayUsd"))?;
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        crate::types::keys::req_key_with::<JsInt>(obj, "machines", &mut out, Self::rule_machines);
        crate::types::keys::req_key_with::<JsNumber>(obj, "spendPerDayUsd", &mut out, Self::rule_spend_per_day_usd);
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
        crate::types::check::num_at_least(x.0 .0, 1.0)?;
        Ok(())
    }
    fn rule_spend_per_day_usd(x: &JsNumber) -> Result<(), crate::types::check::SchemaError> {
        crate::types::check::num_at_least(x.0, 0.0)?;
        Ok(())
    }
}

/// TS: packages/protocol/src/views/place.ts:437 at a3b870a6e.
/// A zod union: the first variant that parses wins, in this order.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(untagged)]
pub enum PlaceCap {
    ComputerCap(ComputerCap),
    CloudCap(CloudCap),
}

crate::zod_union!(PlaceCap { ComputerCap(ComputerCap), CloudCap(CloudCap) }

);

impl Check for PlaceCap {
    fn check(&self) -> Result<(), SchemaError> {
        match self {
            PlaceCap::ComputerCap(v) => v.check(),
            PlaceCap::CloudCap(v) => v.check(),
        }
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        Some(crate::types::check::union_answer(vec![
            crate::types::check::issues_of::<ComputerCap>(value),
            crate::types::check::issues_of::<CloudCap>(value),
        ]))
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        if path.is_empty() {
            return Some(crate::types::check::Zod::Union);
        }
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
            crate::types::check::num_at_least(v.0 .0, 1.0).map_err(|e| e.under("threads"))?;
        }
        if let Some(v) = self.machines {
            crate::types::check::num_at_least(v.0 .0, 1.0).map_err(|e| e.under("machines"))?;
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
        crate::types::keys::opt_key_with::<JsInt>(obj, "threads", &mut out, Self::rule_threads);
        crate::types::keys::opt_key_with::<JsInt>(obj, "machines", &mut out, Self::rule_machines);
        crate::types::keys::opt_key_with::<JsNumber>(obj, "spendPerDayUsd", &mut out, Self::rule_spend_per_day_usd);
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
        if let Some(v) = *x {
            crate::types::check::num_at_least(v.0 .0, 1.0)?;
        }
        Ok(())
    }
    fn rule_machines(x: &Option<JsInt>) -> Result<(), crate::types::check::SchemaError> {
        if let Some(v) = *x {
            crate::types::check::num_at_least(v.0 .0, 1.0)?;
        }
        Ok(())
    }
    fn rule_spend_per_day_usd(x: &Option<JsNumber>) -> Result<(), crate::types::check::SchemaError> {
        if let Some(v) = *x {
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
            crate::types::check::num_at_least(v.0 .0, 0.0).map_err(|e| e.under("maxMachines"))?;
        }
        if let Some(v) = self.max_depth {
            crate::types::check::num_at_least(v.0 .0, 1.0).map_err(|e| e.under("maxDepth"))?;
        }
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        crate::types::keys::opt_key::<bool>(obj, "spawn", &mut out);
        crate::types::keys::opt_key_with::<JsInt>(obj, "maxMachines", &mut out, Self::rule_max_machines);
        crate::types::keys::opt_key_with::<JsInt>(obj, "maxDepth", &mut out, Self::rule_max_depth);
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
        if let Some(v) = *x {
            crate::types::check::num_at_least(v.0 .0, 0.0)?;
        }
        Ok(())
    }
    fn rule_max_depth(x: &Option<JsInt>) -> Result<(), crate::types::check::SchemaError> {
        if let Some(v) = *x {
            crate::types::check::num_at_least(v.0 .0, 1.0)?;
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
            crate::types::check::num_at_least(v.0 .0, 1.0).map_err(|e| e.under("threads"))?;
        }
        if let Some(v) = self.machines {
            crate::types::check::num_at_least(v.0 .0, 1.0).map_err(|e| e.under("machines"))?;
        }
        if let Some(v) = self.spend_per_day_usd {
            crate::types::check::num_at_least(v.0, 0.0).map_err(|e| e.under("spendPerDayUsd"))?;
        }
        if let Some(Some(v)) = self.nap_ms {
            crate::types::check::num_at_least(v.0 .0, 60000.0).map_err(|e| e.under("napMs"))?;
        }
        if let Some(Some(v)) = self.nap_ms {
            crate::types::check::num_at_most(v.0 .0, 10800000.0).map_err(|e| e.under("napMs"))?;
        }
        if let Some(Some(v)) = self.turn_limit_ms {
            crate::types::check::num_at_least(v.0 .0, 3600000.0).map_err(|e| e.under("turnLimitMs"))?;
        }
        if let Some(Some(v)) = self.turn_limit_ms {
            crate::types::check::num_at_most(v.0 .0, 86400000.0).map_err(|e| e.under("turnLimitMs"))?;
        }
        self.spawn.check().map_err(|e| e.under("spawn"))?;
        self.size.check().map_err(|e| e.under("size"))?;
        if let Some(v) = self.delete_after_ms {
            crate::types::check::num_at_least(v.0 .0, 0.0).map_err(|e| e.under("deleteAfterMs"))?;
        }
        if let Some(v) = self.delete_after_ms {
            crate::types::check::num_at_most(v.0 .0, 86400000.0).map_err(|e| e.under("deleteAfterMs"))?;
        }
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        crate::types::keys::opt_key_with::<JsInt>(obj, "threads", &mut out, Self::rule_threads);
        crate::types::keys::opt_key_with::<JsInt>(obj, "machines", &mut out, Self::rule_machines);
        crate::types::keys::opt_key_with::<JsNumber>(obj, "spendPerDayUsd", &mut out, Self::rule_spend_per_day_usd);
        crate::types::keys::dbl_key_with::<JsInt>(obj, "napMs", &mut out, Self::rule_nap_ms);
        crate::types::keys::dbl_key_with::<JsInt>(obj, "turnLimitMs", &mut out, Self::rule_turn_limit_ms);
        crate::types::keys::opt_key::<PlaceSettingsSpawn>(obj, "spawn", &mut out);
        crate::types::keys::opt_key::<PlaceSettingsSize>(obj, "size", &mut out);
        crate::types::keys::opt_key_with::<JsInt>(obj, "deleteAfterMs", &mut out, Self::rule_delete_after_ms);
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
        if let Some(v) = *x {
            crate::types::check::num_at_least(v.0 .0, 1.0)?;
        }
        Ok(())
    }
    fn rule_machines(x: &Option<JsInt>) -> Result<(), crate::types::check::SchemaError> {
        if let Some(v) = *x {
            crate::types::check::num_at_least(v.0 .0, 1.0)?;
        }
        Ok(())
    }
    fn rule_spend_per_day_usd(x: &Option<JsNumber>) -> Result<(), crate::types::check::SchemaError> {
        if let Some(v) = *x {
            crate::types::check::num_at_least(v.0, 0.0)?;
        }
        Ok(())
    }
    fn rule_nap_ms(x: &Option<Option<JsInt>>) -> Result<(), crate::types::check::SchemaError> {
        if let Some(Some(v)) = *x {
            crate::types::check::num_at_least(v.0 .0, 60000.0)?;
        }
        if let Some(Some(v)) = *x {
            crate::types::check::num_at_most(v.0 .0, 10800000.0)?;
        }
        Ok(())
    }
    fn rule_turn_limit_ms(x: &Option<Option<JsInt>>) -> Result<(), crate::types::check::SchemaError> {
        if let Some(Some(v)) = *x {
            crate::types::check::num_at_least(v.0 .0, 3600000.0)?;
        }
        if let Some(Some(v)) = *x {
            crate::types::check::num_at_most(v.0 .0, 86400000.0)?;
        }
        Ok(())
    }
    fn rule_delete_after_ms(x: &Option<JsInt>) -> Result<(), crate::types::check::SchemaError> {
        if let Some(v) = *x {
            crate::types::check::num_at_least(v.0 .0, 0.0)?;
        }
        if let Some(v) = *x {
            crate::types::check::num_at_most(v.0 .0, 86400000.0)?;
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
            crate::types::check::num_at_least(v.0 .0, 1.0).map_err(|e| e.under("threads"))?;
        }
        if let Some(v) = self.machines {
            crate::types::check::num_at_least(v.0 .0, 1.0).map_err(|e| e.under("machines"))?;
        }
        if let Some(v) = self.spend_per_day_usd {
            crate::types::check::num_at_least(v.0, 0.0).map_err(|e| e.under("spendPerDayUsd"))?;
        }
        if let Some(Some(v)) = self.nap_ms {
            crate::types::check::num_at_least(v.0 .0, 60000.0).map_err(|e| e.under("napMs"))?;
        }
        if let Some(Some(v)) = self.nap_ms {
            crate::types::check::num_at_most(v.0 .0, 10800000.0).map_err(|e| e.under("napMs"))?;
        }
        if let Some(Some(v)) = self.turn_limit_ms {
            crate::types::check::num_at_least(v.0 .0, 3600000.0).map_err(|e| e.under("turnLimitMs"))?;
        }
        if let Some(Some(v)) = self.turn_limit_ms {
            crate::types::check::num_at_most(v.0 .0, 86400000.0).map_err(|e| e.under("turnLimitMs"))?;
        }
        self.spawn.check().map_err(|e| e.under("spawn"))?;
        self.size.check().map_err(|e| e.under("size"))?;
        if let Some(v) = self.delete_after_ms {
            crate::types::check::num_at_least(v.0 .0, 0.0).map_err(|e| e.under("deleteAfterMs"))?;
        }
        if let Some(v) = self.delete_after_ms {
            crate::types::check::num_at_most(v.0 .0, 86400000.0).map_err(|e| e.under("deleteAfterMs"))?;
        }
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        crate::types::keys::opt_key_with::<JsInt>(obj, "threads", &mut out, Self::rule_threads);
        crate::types::keys::opt_key_with::<JsInt>(obj, "machines", &mut out, Self::rule_machines);
        crate::types::keys::opt_key_with::<JsNumber>(obj, "spendPerDayUsd", &mut out, Self::rule_spend_per_day_usd);
        crate::types::keys::dbl_key_with::<JsInt>(obj, "napMs", &mut out, Self::rule_nap_ms);
        crate::types::keys::dbl_key_with::<JsInt>(obj, "turnLimitMs", &mut out, Self::rule_turn_limit_ms);
        crate::types::keys::opt_key::<PlaceSettingsSpawn>(obj, "spawn", &mut out);
        crate::types::keys::opt_key::<PlaceSettingsSize>(obj, "size", &mut out);
        crate::types::keys::opt_key_with::<JsInt>(obj, "deleteAfterMs", &mut out, Self::rule_delete_after_ms);
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
        if let Some(v) = *x {
            crate::types::check::num_at_least(v.0 .0, 1.0)?;
        }
        Ok(())
    }
    fn rule_machines(x: &Option<JsInt>) -> Result<(), crate::types::check::SchemaError> {
        if let Some(v) = *x {
            crate::types::check::num_at_least(v.0 .0, 1.0)?;
        }
        Ok(())
    }
    fn rule_spend_per_day_usd(x: &Option<JsNumber>) -> Result<(), crate::types::check::SchemaError> {
        if let Some(v) = *x {
            crate::types::check::num_at_least(v.0, 0.0)?;
        }
        Ok(())
    }
    fn rule_nap_ms(x: &Option<Option<JsInt>>) -> Result<(), crate::types::check::SchemaError> {
        if let Some(Some(v)) = *x {
            crate::types::check::num_at_least(v.0 .0, 60000.0)?;
        }
        if let Some(Some(v)) = *x {
            crate::types::check::num_at_most(v.0 .0, 10800000.0)?;
        }
        Ok(())
    }
    fn rule_turn_limit_ms(x: &Option<Option<JsInt>>) -> Result<(), crate::types::check::SchemaError> {
        if let Some(Some(v)) = *x {
            crate::types::check::num_at_least(v.0 .0, 3600000.0)?;
        }
        if let Some(Some(v)) = *x {
            crate::types::check::num_at_most(v.0 .0, 86400000.0)?;
        }
        Ok(())
    }
    fn rule_delete_after_ms(x: &Option<JsInt>) -> Result<(), crate::types::check::SchemaError> {
        if let Some(v) = *x {
            crate::types::check::num_at_least(v.0 .0, 0.0)?;
        }
        if let Some(v) = *x {
            crate::types::check::num_at_most(v.0 .0, 86400000.0)?;
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
        crate::types::check::leaf(
            path,
            crate::types::check::Zod::Options(&[
                "threads",
                "machines",
                "spend",
                "size",
                "delete-after",
                "nap",
                "turn-limit",
                "spawn",
                "max-depth",
            ]),
        )
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
        crate::types::keys::opt_key::<String>(obj, "key", &mut out);
        crate::types::keys::req_key::<bool>(obj, "trial", &mut out);
        crate::types::keys::req_key::<JsInt>(obj, "machinesAtOnce", &mut out);
        crate::types::keys::req_key::<Vec<String>>(obj, "sizes", &mut out);
        crate::types::keys::opt_key::<JsInt>(obj, "running", &mut out);
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
        crate::types::keys::req_key::<JsInt>(obj, "used", &mut out);
        crate::types::keys::req_key::<JsInt>(obj, "limit", &mut out);
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
        crate::types::keys::req_key::<StartsWindow>(obj, "minute", &mut out);
        crate::types::keys::req_key::<StartsWindow>(obj, "hour", &mut out);
        crate::types::keys::req_key::<StartsWindow>(obj, "day", &mut out);
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
        crate::types::keys::req_key::<CloudPlan>(obj, "plan", &mut out);
        crate::types::keys::opt_key::<CloudStarts>(obj, "starts", &mut out);
        crate::types::keys::opt_key::<JsNumber>(obj, "creditHours", &mut out);
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
        crate::types::keys::req_key::<String>(obj, "word", &mut out);
        crate::types::keys::req_key::<String>(obj, "fix", &mut out);
        crate::types::keys::req_key::<PlaceViewBehindAct>(obj, "act", &mut out);
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
        crate::types::keys::req_key::<JsInt>(obj, "running", &mut out);
        crate::types::keys::req_key::<JsInt>(obj, "room", &mut out);
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
