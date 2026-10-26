// SPDX-License-Identifier: AGPL-3.0-only
//! What Claude Code says of a call's result beyond the words its model read. TS:
//! packages/adapter-claude/src/tool-results.ts at a3b870a6e.

use std::sync::LazyLock;

use regex::Regex;
use serde_json::Value;

use crate::claude::fields::{num, rec, str_arr, string};
use crate::pending::whole_file_hunk;
use crate::port::FilePatch;
use wsp_wire::types::js_number::{JsInt, JsNumber};
use wsp_wire::views::session_events::PatchHunk;

static EXIT_CODE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^Exit code (-?[0-9]+)(?:\n|$)").expect("the exit code pattern is valid"));
const FAILED_COMMAND: &str = "Error: Exit code ";

fn hunk_of(value: &Value) -> Option<PatchHunk> {
    let h = rec(Some(value));
    let lines = str_arr(h?.get("lines"))?;
    let field = |key: &str| num(h?.get(key)).map(|n| JsInt(JsNumber(n)));
    Some(PatchHunk {
        old_start: field("oldStart")?,
        old_lines: field("oldLines")?,
        new_start: field("newStart")?,
        new_lines: field("newLines")?,
        lines,
    })
}

fn patch_of(result: &crate::claude::fields::Obj) -> Option<Vec<FilePatch>> {
    let path = string(result.get("filePath"))?;
    let structured = result.get("structuredPatch")?.as_array()?;
    let mut hunks: Vec<PatchHunk> = structured.iter().filter_map(hunk_of).collect();
    let created = if string(result.get("type")) == Some("create") { string(result.get("content")) } else { None };
    if let Some(content) = created.filter(|c| !c.is_empty()).filter(|_| hunks.is_empty()) {
        hunks.push(whole_file_hunk(content, '+'));
    }
    if hunks.is_empty() {
        return None;
    }
    Some(vec![FilePatch { path: path.to_string(), hunks, moved_to: None }])
}

/// A tool_result block's content as text: a string as it is, a list of blocks as their texts, one per line. tool-results.ts:34.
pub fn flatten_content(value: Option<&Value>) -> String {
    if let Some(text) = string(value) {
        return text.to_string();
    }
    let Some(blocks) = value.and_then(Value::as_array) else {
        return String::new();
    };
    blocks
        .iter()
        .map(|block| string(rec(Some(block)).and_then(|b| b.get("text"))).unwrap_or(""))
        .filter(|text| !text.is_empty())
        .collect::<Vec<_>>()
        .join("\n")
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
    let tool_results = blocks.iter().filter(|b| string(rec(Some(*b)).and_then(|o| o.get("type"))) == Some("tool_result")).count();
    if tool_results != 1 {
        return ResultFacts::default();
    }
    if is_error {
        let failed = tool_use_result.and_then(Value::as_str).is_some_and(|s| s.starts_with(FAILED_COMMAND));
        let code = if failed { EXIT_CODE.captures(text).and_then(|c| c[1].parse::<i64>().ok()) } else { None };
        return ResultFacts { exit_code: code, ..ResultFacts::default() };
    }
    let Some(result) = rec(tool_use_result) else {
        return ResultFacts::default();
    };
    if let Some(patch) = patch_of(result) {
        return ResultFacts { patch: Some(patch), ..ResultFacts::default() };
    }
    let (Some(whole), Some(stdout)) = (num(result.get("persistedOutputSize")), string(result.get("stdout"))) else {
        return ResultFacts::default();
    };
    let stderr = string(result.get("stderr")).unwrap_or("");
    let joined = [stdout, stderr].into_iter().filter(|s| !s.is_empty()).collect::<Vec<_>>().join("\n");
    ResultFacts { text: Some(joined), bytes: Some(whole), ..ResultFacts::default() }
}
