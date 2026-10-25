// SPDX-License-Identifier: AGPL-3.0-only
//! What Claude Code says of a call's result beyond the words its model read. TS:
//! packages/adapter-claude/src/tool-results.ts at a3b870a6e.

use serde_json::Value;

use crate::port::FilePatch;

/// A tool_result block's content as text: a string as it is, an array as the non-empty `text` of each block joined by
/// `\n`, anything else `""`. tool-results.ts:34.
pub fn flatten_content(value: Option<&Value>) -> String {
    todo!()
}

/// The fields a result's delta takes over the words the model read. Each `None` is a field left off the delta.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ResultFacts {
    pub text: Option<String>,
    pub bytes: Option<f64>,
    pub exit_code: Option<i64>,
    pub patch: Option<Vec<FilePatch>>,
}

/// tool-results.ts:45.
pub fn result_facts(blocks: &[Value], text: &str, is_error: bool, tool_use_result: Option<&Value>) -> ResultFacts {
    todo!()
}
