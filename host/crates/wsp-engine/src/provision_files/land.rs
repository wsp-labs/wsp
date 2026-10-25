// SPDX-License-Identifier: AGPL-3.0-only
//! provision-files.ts:56-301 and 400-510 at a4033dc34: which files may land, the read of what already stands there,
//! the landing script and the rows it answers with. Spec: spec/files_land.md.
use indexmap::IndexMap;

use wsp_wire::views::place::PlaceProvisionRow;

use super::{FilesSay, OwnedPaths, ProvisionLanding, StoreRoots};
use crate::errors::Result;
use crate::golden_seam::SkippedPath;
use crate::machine::Machine;

/// provision-files.ts:56. `Kept` is a file the person already had with other content, never written over.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LandOutcome {
    Installed,
    Present,
    Kept,
    Failed,
}

/// provision-files.ts:60 `covers`: the destination itself, or anything under it.
pub fn covers(dest: &str, rel: &str) -> bool {
    todo!()
}

/// provision-files.ts:65 `agentPaths`: one agent's `~/`-relative paths off the catalog (`~/<stateHome>`, its skills
/// folder, its config paths, its MCP files), only those starting `~/`, trailing slashes cut, in that order.
pub fn agent_paths(agent: &wsp_catalog::catalog::AgentEntry) -> Vec<String> {
    todo!()
}

/// provision-files.ts:74 `agentStateFile`: the row is an agent's own (`agent_of_row`) or one of its MCP servers
/// (`parse_mcp_id`), that agent is in the catalog, and `source` is covered by one of its paths under `home`.
pub fn agent_state_file(id: &str, source: &str, home: &str) -> bool {
    todo!()
}

/// provision-files.ts:82.
pub const LAND_MARK: &str = "wsp-land";
/// provision-files.ts:92. A key line with this in place of a digest is a tombstone.
pub const NO_DIGEST: &str = "-";
/// provision-files.ts:167.
pub const STAND_MARK: &str = "wsp-stand";
/// provision-files.ts:168.
pub const LEDGER_MARK: &str = "wsp-ledger";
/// provision-files.ts:176. How long the read of what stands there may take.
pub const STAND_MS: u64 = 120_000;
/// provision-files.ts:232. The exit of a command no shell ran.
pub const NO_SHELL_EXIT: i64 = 127;
/// provision-files.ts:485. How long the files may take to reach that computer and be walked into place.
pub const LAND_MS: u64 = 300_000;

/// provision-files.ts:98 `keyTest`: the `wsp_key` shell function's lines.
pub fn key_test() -> Vec<String> {
    todo!()
}

/// provision-files.ts:105 `onceTest`: the `wsp_once` shell function's lines; with no paths, no `case` line.
pub fn once_test(once: &[String]) -> Vec<String> {
    todo!()
}

/// provision-files.ts:118 `storeRoots(home, stores = {})`: for each catalog agent in catalog order whose id has a
/// store, `(rewritten_rel(stateHome), store)`, unless the store is `<home>/<that rel>`.
pub fn store_roots(home: &str, stores: &IndexMap<String, String>) -> StoreRoots {
    todo!()
}

/// provision-files.ts:127 `rootOf`'s answer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RootOf {
    pub root: String,
    pub sub: String,
    pub moved: bool,
}

/// provision-files.ts:127 `rootOf`: the first root whose folder is `rel` or holds it moves it to the store; else
/// the home.
pub fn root_of(home: &str, roots: &StoreRoots, rel: &str) -> RootOf {
    todo!()
}

/// provision-files.ts:133 `landedAt`: `root` when `sub` is empty, else `root/sub`.
pub fn landed_at(home: &str, roots: &StoreRoots, rel: &str) -> String {
    todo!()
}

/// provision-files.ts:140 `destTest`: the `wsp_dest` shell function's lines.
pub fn dest_test(roots: &StoreRoots) -> Vec<String> {
    todo!()
}

/// provision-files.ts:153 `oncePathsOf`: the dests of the landings marked once, in plan order.
pub fn once_paths_of(lands: &[ProvisionLanding]) -> Vec<String> {
    todo!()
}

/// provision-files.ts:156 `Landed`: one line the landing printed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Landed {
    pub rel: String,
    pub outcome: LandOutcome,
}

/// provision-files.ts:162 `landsOnce`.
pub fn lands_once(rel: &str, once: &[String]) -> bool {
    todo!()
}

/// provision-files.ts:184 `standingScript`. Byte for byte the TS's text.
pub fn standing_script(home: &str) -> String {
    todo!()
}

/// provision-files.ts:212 `Standing`: by home-relative path, the digest standing there (`at`) and the digest the
/// list records (`listed`).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Standing {
    pub at: IndexMap<String, String>,
    pub listed: IndexMap<String, String>,
}

/// provision-files.ts:219 `parseStanding`: records split on NUL, fields on tab; fewer than three fields is no
/// record; a later record for the same path overwrites the earlier.
pub fn parse_standing(stdout: &str) -> Standing {
    todo!()
}

/// provision-files.ts:237 `standingDigests`: `None` for no paths, a rejected exec, exit 124 or 127, or a read with
/// no record in either map.
pub async fn standing_digests(machine: &dyn Machine, home: &str, paths: &[String]) -> Option<Standing> {
    todo!()
}

/// provision-files.ts:250 `landFilesScript(home, once = [], roots = [])`. Byte for byte the TS's text.
pub fn land_files_script(home: &str, once: &[String], roots: &StoreRoots) -> String {
    todo!()
}

/// provision-files.ts:401 `parseLanded`: in the order printed; a line counts only with the mark first, at least
/// four tab fields and a known outcome; the path is every field from the fourth on, joined by tabs.
pub fn parse_landed(stdout: &str) -> Vec<Landed> {
    todo!()
}

/// provision-files.ts:412 `ownerOf`: the longest destination covering `rel`; the first of equal length wins.
pub fn owner_of<'a>(rel: &str, lands: &'a [ProvisionLanding]) -> Option<&'a ProvisionLanding> {
    todo!()
}

/// provision-files.ts:422. Paths named in a note before `and N more`.
pub const NAMED: usize = 3;

/// provision-files.ts:428.
pub const STOOD_LINE: &str = "already there before this run, so it stands as it was";

/// provision-files.ts:432 `mergedIntoLine`.
pub fn merged_into_line(names: &[String]) -> String {
    todo!()
}

/// provision-files.ts:437 `fileLabel`: `landedAt` alone without a label, else `<label> <landedAt>`.
pub fn file_label(label: Option<&str>, home: &str, dest: &str, roots: &StoreRoots) -> String {
    todo!()
}

/// provision-files.ts:442 `filesRows`: one row per planned entry in plan order, then one per landed path no entry
/// covers, in landing order.
pub fn files_rows(lands: &[ProvisionLanding], landed: &[Landed], home: &str, roots: &StoreRoots) -> Vec<PlaceProvisionRow> {
    todo!()
}

/// provision-files.ts:477.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct LandFilesResult {
    pub rows: Vec<PlaceProvisionRow>,
    pub owned: OwnedPaths,
    pub skipped: Vec<SkippedPath>,
}

/// provision-files.ts:490 `landAgentFiles`'s argument.
#[derive(Clone)]
pub struct LandAgentFiles {
    pub home: String,
    pub tar: Vec<u8>,
    pub lands: Vec<ProvisionLanding>,
    pub stood: Vec<String>,
    pub say: FilesSay,
    pub roots: StoreRoots,
}

/// provision-files.ts:490 `landAgentFiles`.
pub async fn land_agent_files(machine: &dyn Machine, o: LandAgentFiles) -> Result<LandFilesResult> {
    todo!()
}
