// SPDX-License-Identifier: AGPL-3.0-only
//! Port of packages/runtime/src/harness-catalog.ts at 65f6b89c7: what each harness's command line accepts at launch,
//! the table the pickers fall back on, and the binary's lists read through it. Spec: spec/harness_catalog.md.
use wsp_adapters::port::HarnessCatalogProbe;
use wsp_wire::views::harness::{HarnessCatalog, HarnessOption};
use wsp_words::exit::HostError;

/// When a thread's agent asks before an MCP tool (slate-mcp.ts `ToolAsks`; wsp-slates takes it from here).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ToolAsks {
    Never,
    Every,
    UnlessReadOnly,
}

impl ToolAsks {
    /// `"never"`, `"every"`, `"unless-read-only"`.
    pub fn as_str(self) -> &'static str {
        todo!()
    }
}

/// The seven table rows, in the TS's order: claude, codex, gemini, opencode, pi, hermes, cursor. Every row reads
/// `source: "table"`, `steers`, `renames` and `images` false, and its bypass mode marked default among its permission
/// modes. harness-catalog.ts:71.
pub fn harness_catalogs() -> Vec<HarnessCatalog> {
    todo!()
}

/// The row whose `harness` is that id. harness-catalog.ts:225.
pub fn harness_catalog(harness: &str) -> Option<HarnessCatalog> {
    todo!()
}

/// "never" with no mode named or at the row's bypass mode; else "unless-read-only" where the row says
/// `readOnlyToolsFree`, else "every". harness-catalog.ts:232.
pub fn tool_asks(harness: Option<&str>, mode: Option<&str>) -> ToolAsks {
    todo!()
}

/// A model a binary's lists refused that the table carries, said as that binary's version not running it; `None`
/// where no lists were read or nothing is refused. harness-catalog.ts:240.
pub fn not_run_refused(lists: Option<&HarnessCatalog>, model: Option<&str>) -> Option<HostError> {
    todo!()
}

/// Models newer than any pin, named and never offered: GPT-6-Astra. harness-catalog.ts:245.
pub fn named_only() -> Vec<HarnessOption> {
    todo!()
}

/// A model as a person reads it. harness-catalog.ts:249.
pub fn model_label(model: &str) -> String {
    todo!()
}

/// The row's own small model where the catalog in front of us still lists it. harness-catalog.ts:264.
pub fn smallest_model(catalog: Option<&HarnessCatalog>) -> Option<String> {
    todo!()
}

/// The binary's lists in the wire shape, the table lending labels and descriptions. harness-catalog.ts:274.
pub fn catalog_from_probe(table: &HarnessCatalog, probe: &HarnessCatalogProbe) -> HarnessCatalog {
    todo!()
}
