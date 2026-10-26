// SPDX-License-Identifier: AGPL-3.0-only
//! protocol/src/slate/paths.ts at 65f6b89c7.
#![allow(unused_variables)]

use serde_json::Value;

use super::*;

pub fn get_slate_value(values: &SlateValues, path: &str) -> Option<Value> {
    todo!()
}

pub fn set_slate_value(values: &SlateValues, path: &str, value: Value) -> Option<SlateValues> {
    todo!()
}

pub fn parse_slate_own_path(path: &str) -> Option<SlateOwnPath> {
    todo!()
}

pub fn slate_step(at: Option<Value>, seg: &PathSeg) -> Option<Value> {
    todo!()
}

pub fn slate_equal(a: Option<&Value>, b: Option<&Value>) -> bool {
    todo!()
}
