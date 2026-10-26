// SPDX-License-Identifier: AGPL-3.0-only
//! protocol/src/slate/expr.ts at 65f6b89c7.
#![allow(unused_variables)]

use serde_json::Value;

use super::*;

pub fn evaluate_slate_expression(expr: &str, ctx: &SlateEvalContext) -> Option<Value> {
    todo!()
}

pub fn resolve_slate_prop(v: &Value, ctx: &SlateEvalContext) -> Option<Value> {
    todo!()
}

pub fn slate_dependencies(expr: &str) -> Vec<String> {
    todo!()
}

pub fn slate_text(v: &Value) -> String {
    todo!()
}
