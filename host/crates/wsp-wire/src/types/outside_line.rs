// SPDX-License-Identifier: AGPL-3.0-only
//! Port of packages/protocol/src/outside-line.ts at a3b870a6e: its zod shapes and plain constants, written by the generator.

use serde::{Deserialize, Serialize};

use crate::types::check::{Check, SchemaError};

/// TS: packages/protocol/src/outside-line.ts:18 at a3b870a6e.
pub const OUTSIDE_HELD: u64 = 50;

/// TS: packages/protocol/src/outside-line.ts:13 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OutsideLine {
    pub title: String,
    pub body: String,
    pub show: bool,
    pub sound: bool,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub id: Option<String>,
}

impl Check for OutsideLine {
    fn check(&self) -> Result<(), SchemaError> {
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        crate::types::keys::req_key::<String>(obj, "title", &mut out);
        crate::types::keys::req_key::<String>(obj, "body", &mut out);
        crate::types::keys::req_key::<bool>(obj, "show", &mut out);
        crate::types::keys::req_key::<bool>(obj, "sound", &mut out);
        crate::types::keys::opt_key::<String>(obj, "id", &mut out);
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "title" => <String as Check>::describe(rest),
                "body" => <String as Check>::describe(rest),
                "show" => <bool as Check>::describe(rest),
                "sound" => <bool as Check>::describe(rest),
                "id" => <Option<String> as Check>::describe(rest),
                _ => None,
            },
        }
    }
}
