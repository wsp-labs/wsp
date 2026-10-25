// SPDX-License-Identifier: AGPL-3.0-only
//! The few items of golden-base.ts, golden-import.ts, golden-tools.ts and golden.ts that the ported modules read,
//! while golden*.ts waits on #2441's ruling. One seam for the whole crate: a slice that needs another golden item
//! adds it here. The constants are the TS values printed by oracle/machines.oracle.ts, not retyped.
use serde::{Deserialize, Serialize};

pub use wsp_wire::views::golden_image::GoldenBaseTool;

/// golden-base.ts:108. One `echo "VERSION <name>: ..."` line per base floor command.
pub const BASE_VERSION_LINES: &str = r#"echo "VERSION curl: $(curl --version 2>/dev/null | grep -m1 -E '[0-9]+\.[0-9]+')"
echo "VERSION uv: $(uv --version 2>/dev/null | grep -m1 -E '[0-9]+\.[0-9]+')"
echo "VERSION python3: $(python3 --version 2>/dev/null | grep -m1 -E '[0-9]+\.[0-9]+')"
echo "VERSION git: $(git --version 2>/dev/null | grep -m1 -E '[0-9]+\.[0-9]+')"
echo "VERSION jq: $(jq --version 2>/dev/null | grep -m1 -E '[0-9]+\.[0-9]+')"
echo "VERSION rg: $(rg --version 2>/dev/null | grep -m1 -E '[0-9]+\.[0-9]+')"
echo "VERSION cc: $(cc --version 2>/dev/null | grep -m1 -E '[0-9]+\.[0-9]+')"
echo "VERSION cmake: $(cmake --version 2>/dev/null | grep -m1 -E '[0-9]+\.[0-9]+')"
echo "VERSION ninja: $(ninja --version 2>/dev/null | grep -m1 -E '[0-9]+\.[0-9]+')"
echo "VERSION fd: $(fd --version 2>/dev/null | grep -m1 -E '[0-9]+\.[0-9]+')"
echo "VERSION sqlite3: $(sqlite3 --version 2>/dev/null | grep -m1 -E '[0-9]+\.[0-9]+')"
echo "VERSION wget: $(wget --version 2>/dev/null | grep -m1 -E '[0-9]+\.[0-9]+')"
echo "VERSION zip: $(zip --version 2>/dev/null | grep -m1 -E '[0-9]+\.[0-9]+')"
echo "VERSION unzip: $(unzip -v 2>/dev/null | grep -m1 -E '[0-9]+\.[0-9]+')"
echo "VERSION xz: $(xz --version 2>/dev/null | grep -m1 -E '[0-9]+\.[0-9]+')"
echo "VERSION rsync: $(rsync --version 2>/dev/null | grep -m1 -E '[0-9]+\.[0-9]+')"
echo "VERSION sshd: $(sh -c 'sshd -V 2>&1' 2>/dev/null | grep -m1 -E '[0-9]+\.[0-9]+')""#;

/// golden-import.ts. The PATH every install and probe runs on.
pub const TOOLS_PATH: &str = r#"/root/.local/bin:/usr/local/sbin:/usr/local/bin:/home/linuxbrew/.linuxbrew/bin:/home/linuxbrew/.linuxbrew/sbin:/root/go/bin:/root/.cargo/bin:/root/.local/share/pnpm:/root/.bun/bin:/usr/sbin:/usr/bin:/sbin:/bin"#;

/// golden-import.ts:821. The environment line every box command runs under.
pub const EXEC_ENV: &str = r#"export HOME=/root USER=root"#;

/// golden-tools.ts:72. 2 GiB, in bytes.
pub const TOOLS_DISK_FLOOR: u64 = 2147483648;

/// golden-tools.ts:44. df's columns.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DfColumn {
    Size,
    Used,
    Free,
}

/// golden-tools.ts:49. `df -Pk <path> | awk 'NR==2{print $2, $4}'`, the columns in the order asked; `path` is
/// written as given, unquoted; `None` is `/root`.
pub fn df_kb_cmd(columns: &[DfColumn], path: Option<&str>) -> String {
    todo!()
}

/// golden-base.ts:114. Each `VERSION <name>: <line>` whose line carries `\d+\.\d+(\.\d+)?`, as that number alone;
/// lines are read with their trailing whitespace trimmed; `<name>` stops at the first colon.
pub fn parse_versions(stdout: &str) -> Vec<GoldenBaseTool> {
    todo!()
}

/// golden.ts:547 `ImportResult`, the fields machine-context's `merge_facts` reads and nothing more.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportResult {
    pub recipe_hash: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub files: Option<ImportFiles>,
    pub tools: Vec<ImportRow>,
    pub agents: Vec<ImportAgentRow>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub retired: Option<Vec<RetiredRow>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub base: Option<Vec<ImportRow>>,
}

/// A tool or base row's outcome (`ToolResult`): its id, label, outcome word and note.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ImportRow {
    pub id: String,
    pub label: String,
    pub outcome: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

/// An agent row's outcome (`AgentResult`): `name` where a tool row says `label`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ImportAgentRow {
    pub id: String,
    pub name: String,
    pub outcome: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct RetiredRow {
    pub id: String,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ImportFiles {
    pub bytes: u64,
    pub skipped: Vec<SkippedPath>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct SkippedPath {
    pub path: String,
    pub note: String,
}

// ---- Read by the rest slice (orphans, drop_template, tool_sizes).

/// golden.ts:378. The idle window a builder is created with: six hours.
pub const BUILDER_IDLE_MS: u64 = 6 * 60 * 60_000;

/// golden.ts:291 `templatesOf`, as a test: the backend says it has templates and offers all four template methods.
pub fn has_templates(backend: &dyn crate::machine::MachineBackend) -> bool {
    todo!()
}

/// golden-import.ts:761.
pub const CATALOG_PREFIX: &str = "tools/catalog/";
/// golden-import.ts:856.
pub const TAP_PREFIX: &str = "tools/brew-tap/";

/// golden-import.ts:801. Tap formulae that only build for macOS and say nothing of it in their metadata.
pub const MACOS_ONLY_FORMULAE: [&str; 4] = ["felixkratz/formulae/sketchybar", "felixkratz/formulae/borders", "koekeishiya/formulae/yabai", "koekeishiya/formulae/skhd"];

/// golden-import.ts:22 `RecipeEntry`, the two fields tool-sizes.ts reads; the golden port widens it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecipeEntry {
    pub rung: String,
    pub id: String,
}

/// golden-import.ts:777.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolSource {
    pub repo: String,
    pub tag: String,
}

/// golden-import.ts:783. What this Mac's Homebrew says about one installed formula.
#[derive(Debug, Clone, PartialEq)]
pub struct BrewFormula {
    pub name: String,
    /// With the tap prefix for a tap formula; the same as name for homebrew/core.
    pub full_name: String,
    /// Runtime dependencies by full name, direct and transitive, each once, first-seen order.
    pub deps: Vec<String>,
    /// The Cellar entry's size on the Mac, when read.
    pub bytes: Option<f64>,
    pub version: Option<String>,
    pub macos_only: bool,
    pub source: Option<ToolSource>,
}

/// golden-import.ts:798. By full name, in insertion order.
pub type BrewTable = indexmap::IndexMap<String, BrewFormula>;

/// golden-import.ts:858. The id starts with `TAP_PREFIX`.
pub fn is_tap(e: &RecipeEntry) -> bool {
    todo!()
}

/// golden-import.ts:862. The id after `tools/brew/`, or `None`.
pub fn formula_of(e: &RecipeEntry) -> Option<String> {
    todo!()
}

/// golden-import.ts:928. A `tools/catalog/` row is the catalog entry by its package when that entry is a tool; a
/// `tools/brew/` row is the formula's manager tool (golden-import.ts:920: `catalog_tool_for(formula)` whose install road
/// is not brew and which is one of the managers' own tools); nothing for any other row.
pub fn catalog_tool_of(e: &RecipeEntry) -> Option<&'static wsp_catalog::catalog::ToolEntry> {
    todo!()
}

/// golden-tools.ts:279 answer.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MissingCommands {
    pub missing: std::collections::HashSet<String>,
    /// Why the check itself could not run (golden-tools' `reasonOf`), where it exited non-zero.
    pub failed: Option<String>,
}

/// golden-tools.ts:279, on the tools PATH with no prefix: one exec under INLINE_EXEC_MS of golden-import's
/// `pathLine(TOOLS_PATH)`, a newline, then `for b in <q bins...>; do command -v "$b" >/dev/null 2>&1 || echo "missing $b"; done`
/// (the exact text pinned by the oracle). `missing` is every stdout line opening `missing `, the rest trimmed.
pub async fn missing_commands(machine: &dyn crate::machine::Machine, bins: &[String]) -> crate::errors::Result<MissingCommands> {
    todo!()
}
