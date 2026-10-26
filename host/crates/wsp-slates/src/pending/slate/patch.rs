// SPDX-License-Identifier: AGPL-3.0-only
//! protocol/src/slate/patch.ts at 65f6b89c7.
#![allow(unused_variables)]

use serde_json::Value;

use super::*;

pub fn apply_slate_patch(doc: Option<&SlateDoc>, values: &SlateValues, ops: &[Value], lines: &Value) -> SlateApplied {
    todo!()
}
