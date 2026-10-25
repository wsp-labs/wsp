// SPDX-License-Identifier: AGPL-3.0-only
//! Port of packages/protocol/src/views/place-setup-words.ts at a3b870a6e: its zod shapes and plain constants, written by the generator.

use serde::{Deserialize, Serialize};

use crate::types::check::{Check, SchemaError};

/// TS: packages/protocol/src/views/place-setup-words.ts:218 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaceEstimate {
    pub needed_bytes: i64,
    pub kept_bytes: i64,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub free_bytes: Option<i64>,
    pub unmeasured: i64,
}

impl Check for PlaceEstimate {
    /// Rules: neededBytes: >= 0; keptBytes: >= 0; freeBytes: >= 0; unmeasured: >= 0.
    fn check(&self) -> Result<(), SchemaError> {
        crate::types::check::num_at_least(self.needed_bytes as f64, 0.0).map_err(|e| e.under("neededBytes"))?;
        crate::types::check::num_at_least(self.kept_bytes as f64, 0.0).map_err(|e| e.under("keptBytes"))?;
        if let Some(v) = self.free_bytes {
            crate::types::check::num_at_least(v as f64, 0.0).map_err(|e| e.under("freeBytes"))?;
        }
        crate::types::check::num_at_least(self.unmeasured as f64, 0.0).map_err(|e| e.under("unmeasured"))?;
        Ok(())
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
