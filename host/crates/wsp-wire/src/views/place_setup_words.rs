// SPDX-License-Identifier: AGPL-3.0-only
//! Port of packages/protocol/src/views/place-setup-words.ts at a3b870a6e: its zod shapes and plain constants, written by the generator.

use serde::{Deserialize, Serialize};

use crate::types::check::{Check, SchemaError};
use crate::types::js_number::JsInt;

/// TS: packages/protocol/src/views/place-setup-words.ts:218 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaceEstimate {
    pub needed_bytes: JsInt,
    pub kept_bytes: JsInt,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub free_bytes: Option<JsInt>,
    pub unmeasured: JsInt,
}

impl Check for PlaceEstimate {
    /// Rules: neededBytes: >= 0; keptBytes: >= 0; freeBytes: >= 0; unmeasured: >= 0.
    fn check(&self) -> Result<(), SchemaError> {
        crate::types::check::num_at_least(self.needed_bytes.0.0, 0.0).map_err(|e| e.under("neededBytes"))?;
        crate::types::check::num_at_least(self.kept_bytes.0.0, 0.0).map_err(|e| e.under("keptBytes"))?;
        if let Some(v) = self.free_bytes {
            crate::types::check::num_at_least(v.0.0, 0.0).map_err(|e| e.under("freeBytes"))?;
        }
        crate::types::check::num_at_least(self.unmeasured.0.0, 0.0).map_err(|e| e.under("unmeasured"))?;
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        match obj.get("neededBytes") { None => out.push(crate::types::check::missing::<Self>("neededBytes")), Some(v) => { match crate::types::check::read::<JsInt>(v) { Ok(x) => { if let Err(e) = Self::rule_needed_bytes(&x) { out.push(e.under("neededBytes")); } }, Err(es) => out.extend(es.into_iter().map(|e| e.under("neededBytes"))), } } }
        match obj.get("keptBytes") { None => out.push(crate::types::check::missing::<Self>("keptBytes")), Some(v) => { match crate::types::check::read::<JsInt>(v) { Ok(x) => { if let Err(e) = Self::rule_kept_bytes(&x) { out.push(e.under("keptBytes")); } }, Err(es) => out.extend(es.into_iter().map(|e| e.under("keptBytes"))), } } }
        if let Some(v) = obj.get("freeBytes") { match crate::types::check::read_present::<JsInt>(v) { Ok(x) => { if let Err(e) = Self::rule_free_bytes(&x) { out.push(e.under("freeBytes")); } }, Err(es) => out.extend(es.into_iter().map(|e| e.under("freeBytes"))), } }
        match obj.get("unmeasured") { None => out.push(crate::types::check::missing::<Self>("unmeasured")), Some(v) => { match crate::types::check::read::<JsInt>(v) { Ok(x) => { if let Err(e) = Self::rule_unmeasured(&x) { out.push(e.under("unmeasured")); } }, Err(es) => out.extend(es.into_iter().map(|e| e.under("unmeasured"))), } } }
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "neededBytes" => <i64 as Check>::describe(rest),
                "keptBytes" => <i64 as Check>::describe(rest),
                "freeBytes" => <Option<i64> as Check>::describe(rest),
                "unmeasured" => <i64 as Check>::describe(rest),
                _ => None,
            },
        }
    }
}
impl PlaceEstimate {
    fn rule_needed_bytes(x: &JsInt) -> Result<(), crate::types::check::SchemaError> {
        crate::types::check::num_at_least((*x).0.0, 0.0)?;
        Ok(())
    }
    fn rule_kept_bytes(x: &JsInt) -> Result<(), crate::types::check::SchemaError> {
        crate::types::check::num_at_least((*x).0.0, 0.0)?;
        Ok(())
    }
    fn rule_free_bytes(x: &Option<JsInt>) -> Result<(), crate::types::check::SchemaError> {
        if let Some(v) = (*x) {
            crate::types::check::num_at_least(v.0.0, 0.0)?;
        }
        Ok(())
    }
    fn rule_unmeasured(x: &JsInt) -> Result<(), crate::types::check::SchemaError> {
        crate::types::check::num_at_least((*x).0.0, 0.0)?;
        Ok(())
    }
}
