// SPDX-License-Identifier: AGPL-3.0-only
//! Port of packages/engine/src/project-state/merge.ts. Spec: spec/project_state_scripts.md.
//!
//! The python3 script the machine runs once a project's files have landed, for rows an agent keeps in a store shared
//! with every project.
use crate::errors::Result;

/// merge.ts:11. What the script prints as its last line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MergeOutput {
    Merged { merged: u64, kept: u64, note: Option<String> },
    Waiting { waiting: String },
}

/// merge.ts:14. The last line of the stdout with its trailing whitespace trimmed. Empty: `the merge script printed
/// nothing`. Not JSON, or JSON of neither shape: `the merge script ended with: <line>`. A string `waiting` wins over
/// counts; `merged` and `kept` must both be JSON numbers; a `note` that is not a string is dropped.
pub fn parse_merge_output(stdout: &str) -> Result<MergeOutput> {
    todo!()
}

/// merge.ts:37. A merge script from its steps, byte for byte as the TS builds it (pinned by the oracle).
pub fn merge_script(from: &str, to: &str, steps: &[String]) -> String {
    todo!()
}

/// merge.ts:68. The step rewriting `cwd` on JSON lines of the files, inside the object `holder` names or the line
/// itself.
pub fn jsonl_cwd_step(files: &[String], holder: Option<&str>) -> String {
    todo!()
}
