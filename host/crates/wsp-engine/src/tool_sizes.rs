// SPDX-License-Identifier: AGPL-3.0-only
//! Port of packages/engine/src/tool-sizes.ts. Spec: spec/tool_sizes.md.
//!
//! What a recipe costs on the builder's disk, judged before anything boots: a catalog row takes the catalog's measured
//! size, any other Homebrew formula its dependency closure from this Mac's own Homebrew, and a row nothing measured a
//! stated default for its kind. Nothing here runs a command.
use std::collections::VecDeque;

use indexmap::{IndexMap, IndexSet};
use serde_json::Value;

use wsp_catalog::catalog::{catalog_entry, catalog_tool_by_road, size_bytes};
use wsp_catalog::roads::{is_road, RoadName};
use wsp_wire::views::golden_image::{package_of, tool_row_prefix};
use wsp_words::js::{is_js_whitespace, js_trim};

use crate::brew_shared::BREW_TOOLCHAIN;
use crate::golden_seam::{
    catalog_tool_of, formula_of, is_tap, BrewFormula, BrewTable, RecipeEntry, ToolSource, CATALOG_PREFIX, MACOS_ONLY_FORMULAE,
    TOOLS_DISK_FLOOR,
};

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
        match self {
            AssumedKind::GoInstall => "a go install",
            AssumedKind::UvTool => "a uv tool",
            AssumedKind::NpmGlobal => "an npm global",
            AssumedKind::Agent => "an agent",
            AssumedKind::Install => "an install",
        }
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
    let (mib, kind) = if e.rung == "agents" {
        (350, AssumedKind::Agent)
    } else if e.id.starts_with(&tool_row_prefix("go")) {
        (500, AssumedKind::GoInstall)
    } else if e.id.starts_with(&tool_row_prefix("uv")) {
        (100, AssumedKind::UvTool)
    } else if ["npm", "pnpm", "bun"].iter().any(|m| e.id.starts_with(&tool_row_prefix(m))) {
        (50, AssumedKind::NpmGlobal)
    } else {
        (100, AssumedKind::Install)
    };
    AssumedSize { bytes: mib * MIB, kind }
}

/// tool-sizes.ts:62. The GitHub repository and tag a formula builds from: `https://github.com/<owner>/<repo>[.git][/rest]`;
/// a release asset (`releases/download/<tag>/...`), a tag archive (`archive/[refs/tags/]<tag>.tar.gz|.zip`), or no rest
/// with the tag beside it. `None` otherwise, an empty tag included only where the TS reads one (spec).
pub fn source_of(url: Option<&str>, tag: Option<&str>) -> Option<ToolSource> {
    let (repo, rest) = github_parts(url?)?;
    let found = release_tag(rest).or_else(|| archive_tag(rest)).or_else(|| if rest.is_empty() { tag.map(str::to_string) } else { None })?;
    Some(ToolSource { repo, tag: found })
}

/// The GitHub url's `owner/repo` and its rest after the repository, read as `^https://github\.com/([^/]+)/([^/]+?)(?:\.git)?(?:/(.*))?$`:
/// the repository is the shortest segment that lets `.git` and the rest match, so a `.git` ending is cut.
fn github_parts(url: &str) -> Option<(String, &str)> {
    let tail = url.strip_prefix("https://github.com/")?;
    let (owner, after) = tail.split_once('/')?;
    if owner.is_empty() {
        return None;
    }
    let (segment, rest) = match after.split_once('/') {
        Some((segment, rest)) => {
            if rest.contains(['\n', '\r', '\u{2028}', '\u{2029}']) {
                return None;
            }
            (segment, rest)
        }
        None => (after, ""),
    };
    if segment.is_empty() {
        return None;
    }
    let repo = match segment.strip_suffix(".git") {
        Some(name) if !name.is_empty() => name,
        _ => segment,
    };
    Some((format!("{owner}/{repo}"), rest))
}

fn release_tag(rest: &str) -> Option<String> {
    let after = rest.strip_prefix("releases/download/")?;
    let (tag, _) = after.split_once('/')?;
    (!tag.is_empty()).then(|| tag.to_string())
}

fn archive_tag(rest: &str) -> Option<String> {
    let after = rest.strip_prefix("archive/")?;
    let name = after.strip_prefix("refs/tags/").unwrap_or(after);
    let stem = name.strip_suffix(".tar.gz").or_else(|| name.strip_suffix(".zip"))?;
    (!stem.is_empty() && !stem.contains('/')).then(|| stem.to_string())
}

fn is_record(v: &Value) -> Option<&serde_json::Map<String, Value>> {
    v.as_object()
}

fn strings(v: Option<&Value>) -> Vec<&str> {
    v.and_then(Value::as_array).map(|a| a.iter().filter_map(Value::as_str).collect()).unwrap_or_default()
}

/// tool-sizes.ts:74. The formulae in `brew info --json=v2 --installed`, one row each, without sizes; anything not of
/// that shape is no rows, and a formula without a name is skipped.
pub fn parse_brew_info(json: &Value) -> Vec<BrewFormula> {
    let Some(formulae) = json.as_object().and_then(|o| o.get("formulae")).and_then(Value::as_array) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for f in formulae {
        let Some(f) = is_record(f) else {
            continue;
        };
        let Some(name) = f.get("name").and_then(Value::as_str) else {
            continue;
        };
        let full_name = f.get("full_name").and_then(Value::as_str).unwrap_or(name);
        let installed = f.get("installed").and_then(Value::as_array).and_then(|a| a.first()).and_then(is_record);
        let runtime: Vec<&str> = installed
            .and_then(|i| i.get("runtime_dependencies"))
            .and_then(Value::as_array)
            .map(|a| a.iter().filter_map(|d| is_record(d)?.get("full_name")?.as_str()).collect())
            .unwrap_or_default();
        let mut deps: Vec<String> = Vec::new();
        for d in strings(f.get("dependencies")).into_iter().chain(runtime) {
            if !deps.iter().any(|x| x == d) {
                deps.push(d.to_string());
            }
        }
        let requirements = f.get("requirements").and_then(Value::as_array).map(Vec::as_slice).unwrap_or(&[]);
        let macos_only = MACOS_ONLY_FORMULAE.contains(&full_name)
            || requirements.iter().any(|r| is_record(r).is_some_and(|r| r.get("name").and_then(Value::as_str) == Some("macos")));
        let stable = f.get("urls").and_then(is_record).and_then(|u| u.get("stable")).and_then(is_record);
        let source =
            source_of(stable.and_then(|s| s.get("url")).and_then(Value::as_str), stable.and_then(|s| s.get("tag")).and_then(Value::as_str));
        let version = installed.and_then(|i| i.get("version")).and_then(Value::as_str).map(str::to_string);
        out.push(BrewFormula { name: name.to_string(), full_name: full_name.to_string(), deps, bytes: None, version, macos_only, source });
    }
    out
}

/// The line-terminator characters a JavaScript `.` refuses.
const DOT_REFUSES: [char; 4] = ['\n', '\r', '\u{2028}', '\u{2029}'];

/// `^(\d+)\s+(.+)$` over one trimmed du line: the digits, then the path. The whitespace run gives characters back to the
/// path as the regex's backtracking does, and the path may not hold a character `.` refuses.
fn du_line(line: &str) -> Option<(&str, &str)> {
    let digits_end = line.find(|c: char| !c.is_ascii_digit()).unwrap_or(line.len());
    let (digits, after) = line.split_at(digits_end);
    if digits.is_empty() {
        return None;
    }
    let ws_end = after.len() - after.trim_start_matches(is_js_whitespace).len();
    if ws_end == 0 {
        return None;
    }
    let mut starts: Vec<usize> = after.char_indices().map(|(i, _)| i).take_while(|&i| i < ws_end).skip(1).collect();
    starts.push(ws_end);
    starts
        .into_iter()
        .rev()
        .map(|start| &after[start..])
        .find(|path| !path.is_empty() && !path.contains(DOT_REFUSES))
        .map(|path| (digits, path))
}

/// The name a du line is filed under when the caller names none: the last part of its path.
fn du_key(path: &str) -> String {
    path.rsplit('/').next().unwrap_or(path).to_string()
}

/// tool-sizes.ts:99. `du -sk` lines: bytes (kB * 1024) by the last path segment, trailing slashes cut, or by whatever
/// `key_of` calls the path; later lines of the same key overwrite earlier ones, keeping the first one's place.
pub fn parse_du(text: &str, key_of: Option<&dyn Fn(&str) -> String>) -> IndexMap<String, f64> {
    let mut out = IndexMap::new();
    for line in text.split('\n') {
        let Some((digits, path)) = du_line(js_trim(line)) else {
            continue;
        };
        let path = path.trim_end_matches('/');
        let key = match key_of {
            Some(key_of) => key_of(path),
            None => du_key(path),
        };
        out.insert(key, digits.parse::<f64>().unwrap_or(f64::NAN) * 1024.0);
    }
    out
}

/// tool-sizes.ts:110. The Mac's formulae by full name, each with its Cellar size by its short name where du read one.
pub fn brew_table(json: &Value, du: &IndexMap<String, f64>) -> BrewTable {
    let mut out = BrewTable::new();
    for f in parse_brew_info(json) {
        let bytes = du.get(&f.name).copied();
        let key = f.full_name.clone();
        out.insert(key, BrewFormula { bytes: bytes.or(f.bytes), ..f });
    }
    out
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

/// A formula's bytes: the catalog's measured size where the catalog carries it (unmeasured gives none, never the Mac's),
/// else this Mac's Cellar size.
fn formula_bytes(name: &str, brew: &BrewTable) -> Option<f64> {
    match catalog_tool_by_road(RoadName::Brew, name) {
        Some(entry) => size_bytes(&entry.size).map(|b| b as f64),
        None => brew.get(name).and_then(|f| f.bytes),
    }
}

/// The formula and every formula it depends on, by full name, Homebrew's toolchain left out; a formula the catalog
/// carries is a leaf, since its measured size already holds its runtime dependencies.
fn closure_of(name: &str, brew: &BrewTable) -> IndexSet<String> {
    let mut seen = IndexSet::from([name.to_string()]);
    let mut queue = VecDeque::from([name.to_string()]);
    while let Some(n) = queue.pop_front() {
        if catalog_tool_by_road(RoadName::Brew, &n).is_some() {
            continue;
        }
        let deps = brew.get(&n).map(|f| f.deps.as_slice()).unwrap_or(&[]);
        for d in deps {
            if BREW_TOOLCHAIN.contains(&d.as_str()) || seen.contains(d) {
                continue;
            }
            seen.insert(d.clone());
            queue.push_back(d.clone());
        }
    }
    seen
}

/// The catalog's measured size for a row: a catalog row's own, or the size of the catalog tool that installs the same
/// package by the same road as this Mac's row (a formula by brew, a global by npm).
fn catalog_size(e: &RecipeEntry) -> Option<ToolSize> {
    let pkg = package_of(&e.id);
    let bytes = if e.id.starts_with(CATALOG_PREFIX) {
        size_bytes(catalog_entry(&pkg)?.size())
    } else {
        let road = is_road(e.id.split('/').nth(1));
        let tool = catalog_tool_of(e).or_else(|| road.and_then(|r| catalog_tool_by_road(r, &pkg)));
        size_bytes(&tool?.size)
    };
    bytes.map(|b| ToolSize { bytes: b as f64, road: SizeRoad::Measured, deps: 0 })
}

/// tool-sizes.ts:162. Nothing for a tap. The catalog's measurement where it carries the row (deps 0); else a formula
/// this Mac's Homebrew sized, with its closure (Homebrew's toolchain left out, a catalog formula a leaf) summed;
/// nothing for a row nothing measured.
pub fn tool_size(e: &RecipeEntry, brew: &BrewTable) -> Option<ToolSize> {
    if is_tap(&e.id) {
        return None;
    }
    if let Some(known) = catalog_size(e) {
        return Some(known);
    }
    let formula = formula_of(&e.id)?;
    formula_bytes(&formula, brew)?;
    let members = closure_of(&formula, brew);
    let bytes = members.iter().map(|m| formula_bytes(m, brew).unwrap_or(0.0)).sum();
    Some(ToolSize { bytes, road: SizeRoad::Mac, deps: members.len() - 1 })
}

/// tool-sizes.ts:177. The catalog entry's measured size by the id after its first `/`.
pub fn agent_size(e: &RecipeEntry) -> Option<u64> {
    let key = match e.id.find('/') {
        Some(i) => &e.id[i + 1..],
        None => e.id.as_str(),
    };
    size_bytes(catalog_entry(key)?.size())
}
