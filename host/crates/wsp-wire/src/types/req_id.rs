// SPDX-License-Identifier: AGPL-3.0-only
//! The id a request carries and its reply echoes: zod's `z.union([z.string(), z.number()])`. A number is the double
//! JavaScript reads, so `5.0` and `-0` are written `5` and `0`.

use serde::de::{Deserializer, Error};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::types::check::{leaf, Check, Zod};
use crate::types::js_number::JsNumber;

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(untagged)]
pub enum ReqId {
    Num(JsNumber),
    Str(String),
}

impl<'de> Deserialize<'de> for ReqId {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        match Value::deserialize(d)? {
            Value::Number(n) => Ok(ReqId::Num(JsNumber(n.as_f64().unwrap_or(0.0)))),
            Value::String(s) => Ok(ReqId::Str(s)),
            _ => Err(D::Error::custom("data did not match any variant of untagged enum ReqId")),
        }
    }
}

impl From<u64> for ReqId {
    fn from(n: u64) -> Self {
        ReqId::Num(JsNumber(n as f64))
    }
}

impl From<&str> for ReqId {
    fn from(s: &str) -> Self {
        ReqId::Str(s.to_owned())
    }
}

impl Check for ReqId {
    fn describe(path: &[Value]) -> Option<Zod> {
        leaf(path, Zod::Union)
    }
}
