// SPDX-License-Identifier: AGPL-3.0-only
//! The catalog's rows: the eleven agent modules of packages/catalog/src/agents/*.ts and the tool rows of
//! packages/catalog/src/catalog.ts:239-312, at a3b870a6e. Data, copied by script from the oracle fixture
//! tests/fixtures/catalog.json, never typed by hand; the spec is spec/catalog.md.
use crate::catalog::{AgentEntry, Size, ToolEntry};

/// agents/entry.ts:6.
pub const AGENTS_MD: &str = "AGENTS.md";
/// agents/entry.ts:9.
pub const SET_UP_WSP: &str = "set up wsp for me";

/// `mib` MiB as df read them on 2026-09-05 (agents/entry.ts:13).
pub fn df_size(mib: u64) -> Size {
    todo!()
}

/// agents/entry.ts:20.
pub fn not_measured() -> Size {
    todo!()
}

/// claude, codex, gemini, opencode, pi, hermes, crush, qwen, goose, amp, cursor (agents/index.ts:46).
pub fn agent_modules() -> Vec<AgentEntry> {
    todo!()
}

/// The tool rows in catalog order, curl first (catalog.ts:239).
pub fn tool_rows() -> Vec<ToolEntry> {
    todo!()
}
