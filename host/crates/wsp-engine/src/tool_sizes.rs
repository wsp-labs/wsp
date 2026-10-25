// SPDX-License-Identifier: AGPL-3.0-only
//! Port of packages/engine/src/tool-sizes.ts. Spec: spec/tool_sizes.md.
//!
//! What a recipe costs on the builder's disk, judged before anything boots: a catalog row takes the catalog's measured
//! size, any other Homebrew formula its dependency closure from this Mac's own Homebrew, and a row nothing measured a
//! stated default for its kind. Nothing here runs a command.
use indexmap::IndexMap;
use serde_json::Value;

use crate::golden_seam::{BrewFormula, BrewTable, RecipeEntry, ToolSource, TOOLS_DISK_FLOOR};

const MIB: u64 = 1024 * 1024;

/// tool-sizes.ts:17. Root disk asked for every builder and fork, Solari's cap.
pub const BUILDER_DISK_GB: u64 = 20;
/// tool-sizes.ts:21. What df said was free on a 20 GB builder after the base stage.
pub const BUILDER_FREE_BYTES: u64 = 17570 * MIB;
/// tool-sizes.ts:24. What the recipe may take.
pub const DISK_ROOM_BYTES: u64 = BUILDER_FREE_BYTES - TOOLS_DISK_FLOOR;
/// tool-sizes.ts:27. Homebrew's checkout with its own glibc and gcc.
pub const BREW_TOOLCHAIN_BYTES: u64 = 1024 * MIB;

/// tool-sizes.ts:37. Where a default came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AssumedKind {
    /// "a go install", 500 MiB.
    GoInstall,
    /// "a uv tool", 100 MiB.
    UvTool,
    /// "an npm global", 50 MiB.
    NpmGlobal,
    /// "an agent", 350 MiB.
    Agent,
    /// "an install", 100 MiB.
    Install,
}

impl AssumedKind {
    /// The words the TS carries in `kind`.
    pub fn words(self) -> &'static str {
        todo!()
    }
}

/// tool-sizes.ts:35.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AssumedSize {
    pub bytes: u64,
    pub kind: AssumedKind,
}

/// tool-sizes.ts:43. An `agents` rung row is an agent; else by the id's prefix: `tool_row_prefix("go")`, `("uv")`, any
/// of npm, pnpm, bun; anything else an install.
pub fn assumed_size(e: &RecipeEntry) -> AssumedSize {
    todo!()
}

/// tool-sizes.ts:62. The GitHub repository and tag a formula builds from: `https://github.com/<owner>/<repo>[.git][/rest]`;
/// a release asset (`releases/download/<tag>/...`), a tag archive (`archive/[refs/tags/]<tag>.tar.gz|.zip`), or no rest
/// with the tag beside it. `None` otherwise, an empty tag included only where the TS reads one (spec).
pub fn source_of(url: Option<&str>, tag: Option<&str>) -> Option<ToolSource> {
    todo!()
}

/// tool-sizes.ts:74. The formulae in `brew info --json=v2 --installed`, one row each, without sizes; anything not of
/// that shape is no rows, and a formula without a name is skipped.
pub fn parse_brew_info(json: &Value) -> Vec<BrewFormula> {
    todo!()
}

/// tool-sizes.ts:99. `du -sk` lines: bytes (kB * 1024) by the last path segment, trailing slashes cut, or by whatever
/// `key_of` calls the path; later lines of the same key overwrite earlier ones, keeping the first one's place.
pub fn parse_du(text: &str, key_of: Option<&dyn Fn(&str) -> String>) -> IndexMap<String, f64> {
    todo!()
}

/// tool-sizes.ts:110. The Mac's formulae by full name, each with its Cellar size by its short name where du read one.
pub fn brew_table(json: &Value, du: &IndexMap<String, f64>) -> BrewTable {
    todo!()
}

/// tool-sizes.ts:121. Where the number came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SizeRoad {
    Measured,
    Mac,
}

/// tool-sizes.ts:119.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ToolSize {
    pub bytes: f64,
    pub road: SizeRoad,
    /// Dependencies counted in besides the row's own formula.
    pub deps: usize,
}

/// tool-sizes.ts:162. Nothing for a tap. The catalog's measurement where it carries the row (deps 0); else a formula
/// this Mac's Homebrew sized, with its closure (Homebrew's toolchain left out, a catalog formula a leaf) summed;
/// nothing for a row nothing measured.
pub fn tool_size(e: &RecipeEntry, brew: &BrewTable) -> Option<ToolSize> {
    todo!()
}

/// tool-sizes.ts:177. The catalog entry's measured size by the id after its first `/`.
pub fn agent_size(e: &RecipeEntry) -> Option<u64> {
    todo!()
}
