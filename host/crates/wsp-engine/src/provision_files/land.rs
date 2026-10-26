// SPDX-License-Identifier: AGPL-3.0-only
//! provision-files.ts:56-301 and 400-510 at a4033dc34: which files may land, the read of what already stands there,
//! the landing script and the rows it answers with. Spec: spec/files_land.md.
use std::sync::Arc;

use indexmap::IndexMap;
use wsp_catalog::catalog::catalog_agents;
use wsp_catalog::roads::rewritten_rel;
use wsp_catalog::skills::skills_dir_of;
use wsp_wire::types::shell_quote::shell_quote;
use wsp_wire::views::golden_image::{agent_of_row, MCP_ID_PREFIX};
use wsp_wire::views::place::{PlaceProvisionKind, PlaceProvisionRow, PlaceProvisionRowOutcome};
use wsp_wire::wire::daemon::EXEC_DEADLINE_EXIT;
use wsp_words::project_path::place_provision_paths;
use wsp_words::views::place_setup_words::{provision_landed_line, provision_shipped_line, Shipped};

use super::{FilesSay, OwnedHow, OwnedPaths, ProvisionLanding, StoreRoots};
use crate::errors::{EngineError, Result};
use crate::exec_detached::INLINE_EXEC_MS;
use crate::golden_seam::{parse_mcp_id, SkippedPath};
use crate::machine::{ExecOptions, Machine, RunOptions};
use crate::vault::{import_into, UploadProgress, VaultOptions};

const TAB: &str = r"\t";
const NL: &str = r"\n";
const NUL: &str = r"\0";

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
    rel == dest || rel.starts_with(&format!("{dest}/"))
}

/// provision-files.ts:65 `agentPaths`: one agent's `~/`-relative paths off the catalog (`~/<stateHome>`, its skills
/// folder, its config paths, its MCP files), only those starting `~/`, trailing slashes cut, in that order.
pub fn agent_paths(agent: &wsp_catalog::catalog::AgentEntry) -> Vec<String> {
    let mut paths = vec![format!("~/{}", agent.state_home), skills_dir_of(&agent.skill_roots)];
    paths.extend(agent.config_paths.iter().cloned());
    paths.extend(agent.mcp.iter().flat_map(|m| m.files.iter().cloned()));
    paths.into_iter().filter(|p| p.starts_with("~/")).map(|p| p.trim_end_matches('/').to_string()).collect()
}

/// provision-files.ts:74 `agentStateFile`: the row is an agent's own (`agent_of_row`) or one of its MCP servers
/// (`parse_mcp_id`), that agent is in the catalog, and `source` is covered by one of its paths under `home`.
pub fn agent_state_file(id: &str, source: &str, home: &str) -> Result<bool> {
    let agent = match agent_of_row(id) {
        Some(agent) => agent,
        None => match parse_mcp_id(id)? {
            Some(m) => m.agent,
            None => return Ok(false),
        },
    };
    let Some(entry) = catalog_agents().into_iter().find(|a| a.id == agent) else {
        return Ok(false);
    };
    Ok(agent_paths(entry).iter().any(|p| covers(&format!("{home}/{}", &p[2..]), source)))
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
    vec![
        "wsp_key() {".to_string(),
        format!(r#"  case "$1" in ({}*) return 0 ;; esac"#, shell_quote(MCP_ID_PREFIX)),
        "  return 1".to_string(),
        "}".to_string(),
    ]
}

/// provision-files.ts:105 `onceTest`: the `wsp_once` shell function's lines; with no paths, no `case` line.
pub fn once_test(once: &[String]) -> Vec<String> {
    let patterns: Vec<String> = once.iter().flat_map(|dest| [shell_quote(dest), format!("{}/*", shell_quote(dest))]).collect();
    let mut lines = vec!["wsp_once() {".to_string()];
    if !patterns.is_empty() {
        lines.push(format!(r#"  case "$1" in ({}) return 0 ;; esac"#, patterns.join("|")));
    }
    lines.push("  return 1".to_string());
    lines.push("}".to_string());
    lines
}

/// provision-files.ts:118 `storeRoots(home, stores = {})`: for each catalog agent in catalog order whose id has a
/// store, `(rewritten_rel(stateHome), store)`, unless the store is `<home>/<that rel>`.
pub fn store_roots(home: &str, stores: &IndexMap<String, String>) -> StoreRoots {
    catalog_agents()
        .into_iter()
        .filter_map(|a| {
            let store = stores.get(&a.id)?;
            let from = rewritten_rel(&a.state_home, None);
            (store != &format!("{home}/{from}")).then(|| (from, store.clone()))
        })
        .collect()
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
    match roots.iter().find(|(from, _)| rel == from.as_str() || rel.starts_with(&format!("{from}/"))) {
        Some((from, store)) => RootOf { root: store.clone(), sub: rel.get(from.len() + 1..).unwrap_or_default().to_string(), moved: true },
        None => RootOf { root: home.to_string(), sub: rel.to_string(), moved: false },
    }
}

/// provision-files.ts:133 `landedAt`: `root` when `sub` is empty, else `root/sub`.
pub fn landed_at(home: &str, roots: &StoreRoots, rel: &str) -> String {
    let r = root_of(home, roots, rel);
    if r.sub.is_empty() {
        r.root
    } else {
        format!("{}/{}", r.root, r.sub)
    }
}

/// provision-files.ts:140 `destTest`: the `wsp_dest` shell function's lines.
pub fn dest_test(roots: &StoreRoots) -> Vec<String> {
    let mut lines = vec!["wsp_dest() {".to_string(), r#"  case "$1" in"#.to_string()];
    for (from, store) in roots {
        lines.push(format!(
            "    ({}/*) root={}; sub=${{1#{}/}}; top=\"$home\"/{} ;;",
            shell_quote(from),
            shell_quote(store),
            shell_quote(from),
            shell_quote(from)
        ));
    }
    lines.extend([
        "    (*) root=$home; sub=$1; top=$home ;;".to_string(),
        "  esac".to_string(),
        r#"  dest="$root/$sub""#.to_string(),
        "}".to_string(),
    ]);
    lines
}

/// provision-files.ts:153 `oncePathsOf`: the dests of the landings marked once, in plan order.
pub fn once_paths_of(lands: &[ProvisionLanding]) -> Vec<String> {
    lands.iter().filter(|l| l.once).map(|l| l.dest.clone()).collect()
}

/// provision-files.ts:156 `Landed`: one line the landing printed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Landed {
    pub rel: String,
    pub outcome: LandOutcome,
}

/// provision-files.ts:162 `landsOnce`.
pub fn lands_once(rel: &str, once: &[String]) -> bool {
    once.iter().any(|dest| covers(dest, rel))
}

/// provision-files.ts:184 `standingScript`. Byte for byte the TS's text.
pub fn standing_script(home: &str) -> String {
    let at = place_provision_paths(home);
    let q = |s: &str| shell_quote(s);
    [
        "set -u".to_string(),
        format!("home={}; ledger={}; list={}", q(home), q(&at.landed), q(&at.asked)),
        r#"mkdir -p "$(dirname "$list")" || exit 1"#.to_string(),
        r#"cat > "$list" || exit 1"#.to_string(),
        r#"[ -s "$list" ] || exit 0"#.to_string(),
        r#"cd "$home" || exit 1"#.to_string(),
        r#"xargs -0 sha256sum -z -- < "$list" 2>/dev/null | while IFS= read -r -d "" rec; do"#.to_string(),
        "  p=${rec#* }".to_string(),
        format!(r#"  printf 'wsp-stand{TAB}%s{TAB}%s{NUL}' "${{rec%% *}}" "${{p#[ *]}}""#),
        "done".to_string(),
        r#"[ -f "$ledger" ] || ledger=/dev/null"#.to_string(),
        format!(r#"xargs -0 printf '%s{NL}' < "$list" | awk -F"{TAB}" 'NR==FNR {{ want[$0]=1; next }} want[$1] && !seen[$1]++ {{ printf "wsp-ledger{TAB}%s{TAB}%s{NL}", $1, $3 }}' - "$ledger" | tr '{NL}' '{NUL}'"#),
        r#"rm -f "$list""#.to_string(),
        "exit 0".to_string(),
    ]
    .join("\n")
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
    let mut standing = Standing::default();
    for record in stdout.split('\0') {
        let words: Vec<&str> = record.split('\t').collect();
        if words.len() < 3 {
            continue;
        }
        if words[0] == STAND_MARK {
            standing.at.insert(words[2..].join("\t"), words[1].to_string());
        }
        if words[0] == LEDGER_MARK {
            standing.listed.insert(words[1].to_string(), words[2].to_string());
        }
    }
    standing
}

/// provision-files.ts:237 `standingDigests`: `None` for no paths, a rejected exec, exit 124 or 127, or a read with
/// no record in either map.
pub async fn standing_digests(machine: &dyn Machine, home: &str, paths: &[String]) -> Option<Standing> {
    if paths.is_empty() {
        return None;
    }
    let stdin: Vec<u8> = paths.iter().flat_map(|p| p.bytes().chain(std::iter::once(0))).collect();
    let res = machine
        .exec(&standing_script(home), ExecOptions { timeout_ms: Some(STAND_MS), stdin: Some(stdin), ..Default::default() })
        .await
        .ok()?;
    if res.exit_code.get() == EXEC_DEADLINE_EXIT || res.exit_code.get() == NO_SHELL_EXIT {
        return None;
    }
    let standing = parse_standing(&res.stdout);
    if standing.at.is_empty() && standing.listed.is_empty() {
        None
    } else {
        Some(standing)
    }
}

/// provision-files.ts:250 `landFilesScript(home, once = [], roots = [])`. Byte for byte the TS's text.
pub fn land_files_script(home: &str, once: &[String], roots: &StoreRoots) -> String {
    let at = place_provision_paths(home);
    let q = |s: &str| shell_quote(s);
    let mut lines: Vec<String> = vec!["set -u".to_string()];
    lines.extend(once_test(once));
    lines.extend(dest_test(roots));
    lines.extend([
        format!("home={}; stage={}; ledger={}; landing={}", q(home), q(&at.staging), q(&at.landed), q(&at.landing)),
        r#"canon=$(readlink -f -- "$home") || exit 1"#.to_string(),
        r#"mkdir -p "$(dirname "$landing")" || exit 1"#.to_string(),
        r#": > "$landing" || exit 1"#.to_string(),
        r#"cd "$stage" || exit 1"#.to_string(),
        r#"find . -type f -print | while IFS= read -r p; do"#.to_string(),
        "  rel=${p#./}".to_string(),
        r#"  src="$stage/$rel"; wsp_dest "$rel""#.to_string(),
        r#"  s=$(sha256sum "$src" | cut -d" " -f1)"#.to_string(),
        r#"  if [ ! -e "$dest" ] || { [ -f "$dest" ] && [ ! -s "$dest" ]; }; then act=installed"#.to_string(),
        r#"  elif wsp_once "$rel"; then act=present"#.to_string(),
        "  else".to_string(),
        r#"    d=$(sha256sum "$dest" 2>/dev/null | cut -d" " -f1)"#.to_string(),
        format!(r#"    known=$(wsp_rel="$rel" awk -F"{TAB}" '$1==ENVIRON["wsp_rel"] {{ print $2" "$3; exit }}' "$ledger" 2>/dev/null)"#),
        r#"    if [ "$d" = "$s" ]; then act=present"#.to_string(),
        r#"    elif [ -n "$known" ] && [ "$d" = "${known##* }" ]; then"#.to_string(),
        r#"      if [ "$s" = "${known%% *}" ]; then act=present; else act=installed; fi"#.to_string(),
        "    else act=kept; fi".to_string(),
        "  fi".to_string(),
        r#"  if [ "$act" = installed ]; then"#.to_string(),
        r#"    mkdir -p "$(dirname "$dest")" && cp -p "$src" "$dest" || act=failed"#.to_string(),
        "  fi".to_string(),
        r#"  old="$home/$rel""#.to_string(),
        r#"  if [ "$dest" != "$old" ] && [ "$act" != failed ] && [ -f "$old" ] && [ "$(readlink -f -- "$old")" = "$canon/$rel" ]; then"#.to_string(),
        format!(r#"    was=$(wsp_rel="$rel" awk -F"{TAB}" '$1==ENVIRON["wsp_rel"] {{ print $3; exit }}' "$ledger" 2>/dev/null)"#),
        r#"    if [ -n "$was" ] && [ "$(sha256sum "$old" | cut -d" " -f1)" = "$was" ] && rm -f -- "$old"; then"#.to_string(),
        r#"      d=$(dirname "$old"); while [ "$d" != "$top" ] && rmdir -- "$d" 2>/dev/null; do d=$(dirname "$d"); done"#.to_string(),
        "    fi".to_string(),
        "  fi".to_string(),
        format!(r#"  printf 'wsp-land{TAB}%s{TAB}%s{TAB}%s{NL}' "$act" "$s" "$rel""#),
        format!(r#"  if [ "$act" != kept ] && [ "$act" != failed ] && ! wsp_once "$rel"; then printf "%s{TAB}%s{NL}" "$rel" "$s" >> "$landing"; fi"#),
        "done".to_string(),
        "exit 0".to_string(),
    ]);
    lines.join("\n")
}

/// provision-files.ts:401 `parseLanded`: in the order printed; a line counts only with the mark first, at least
/// four tab fields and a known outcome; the path is every field from the fourth on, joined by tabs.
pub fn parse_landed(stdout: &str) -> Vec<Landed> {
    stdout
        .split('\n')
        .filter_map(|line| {
            let words: Vec<&str> = line.split('\t').collect();
            if words.len() < 4 || words[0] != LAND_MARK {
                return None;
            }
            let outcome = match words[1] {
                "installed" => LandOutcome::Installed,
                "present" => LandOutcome::Present,
                "kept" => LandOutcome::Kept,
                "failed" => LandOutcome::Failed,
                _ => return None,
            };
            Some(Landed { rel: words[3..].join("\t"), outcome })
        })
        .collect()
}

/// provision-files.ts:412 `ownerOf`: the longest destination covering `rel`; the first of equal length wins.
pub fn owner_of<'a>(rel: &str, lands: &'a [ProvisionLanding]) -> Option<&'a ProvisionLanding> {
    let mut best: Option<&ProvisionLanding> = None;
    for l in lands {
        if !covers(&l.dest, rel) {
            continue;
        }
        match best {
            Some(b) if b.dest.len() >= l.dest.len() => {}
            _ => best = Some(l),
        }
    }
    best
}

/// provision-files.ts:422. Paths named in a note before `and N more`.
pub const NAMED: usize = 3;

/// provision-files.ts:428.
pub const STOOD_LINE: &str = "already there before this run, so it stands as it was";

/// provision-files.ts:432 `mergedIntoLine`.
pub fn merged_into_line(names: &[String]) -> String {
    if names.is_empty() {
        "none of this computer's servers merged into it".to_string()
    } else {
        format!("this computer's {} merged into it", names.join(", "))
    }
}

/// provision-files.ts:437 `fileLabel`: `landedAt` alone without a label, else `<label> <landedAt>`.
pub fn file_label(label: Option<&str>, home: &str, dest: &str, roots: &StoreRoots) -> String {
    match label {
        None => landed_at(home, roots, dest),
        Some(label) => format!("{label} {}", landed_at(home, roots, dest)),
    }
}

/// provision-files.ts:442 `filesRows`: one row per planned entry in plan order, then one per landed path no entry
/// covers, in landing order.
pub fn files_rows(lands: &[ProvisionLanding], landed: &[Landed], home: &str, roots: &StoreRoots) -> Vec<PlaceProvisionRow> {
    let mut by_owner: IndexMap<String, Vec<&Landed>> = IndexMap::new();
    let mut loose: Vec<&Landed> = Vec::new();
    for l in landed {
        match owner_of(&l.rel, lands) {
            None => loose.push(l),
            Some(owner) => by_owner.entry(owner.dest.clone()).or_default().push(l),
        }
    }
    let mut rows: Vec<PlaceProvisionRow> = lands
        .iter()
        .map(|l| {
            let under: &[&Landed] = by_owner.get(&l.dest).map_or(&[], |v| v.as_slice());
            row_of(format!("files/{}", l.dest), Some(l.label.as_str()), &l.dest, under, l.once, home, roots)
        })
        .collect();
    rows.extend(loose.iter().map(|l| row_of(format!("files/{}", l.rel), None, &l.rel, &[*l], false, home, roots)));
    rows
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
pub async fn land_agent_files(machine: &Arc<dyn Machine>, o: LandAgentFiles) -> Result<LandFilesResult> {
    let at = place_provision_paths(&o.home);
    machine
        .exec(&format!("rm -rf {}", shell_quote(&at.staging)), ExecOptions { timeout_ms: Some(INLINE_EXEC_MS), ..Default::default() })
        .await?;
    let say = o.say.clone();
    let opts = VaultOptions {
        overlay: true,
        timeout_ms: Some(LAND_MS),
        tmp_dir: Some(at.dir.clone()),
        on_part: Some(Arc::new(move |p: UploadProgress| {
            let shipped = Shipped {
                part: f64::from(p.part),
                parts: f64::from(p.parts),
                bytes: p.bytes as f64,
                total: p.total as f64,
                pieces: p.pieces.map(f64::from),
            };
            say(&provision_shipped_line(&shipped));
        })),
        ..Default::default()
    };
    import_into(machine.clone(), &o.tar, &at.staging, &opts).await?;
    let res = machine
        .run(&land_files_script(&o.home, &once_paths_of(&o.lands), &o.roots), RunOptions { deadline_ms: LAND_MS, ..Default::default() })
        .await?;
    if res.exit_code.get() != 0 {
        return Err(EngineError::Other {
            message: format!(
                "the agents' files did not land on {} (exit {}): {}",
                machine.id(),
                res.exit_code.get(),
                last_units(&res.stderr, 300)
            ),
            code: None,
        });
    }
    let walked = parse_landed(&res.stdout);
    (o.say)(&provision_landed_line(walked.len() as f64));
    let mut landed = walked;
    landed.extend(o.stood.iter().map(|rel| Landed { rel: rel.clone(), outcome: LandOutcome::Present }));
    let rows = files_rows(&o.lands, &landed, &o.home, &o.roots);
    let mut owned: OwnedPaths = IndexMap::new();
    for l in &landed {
        let how = match l.outcome {
            LandOutcome::Installed => OwnedHow::Installed,
            LandOutcome::Present => OwnedHow::Present,
            LandOutcome::Kept | LandOutcome::Failed => continue,
        };
        owned.insert(landed_at(&o.home, &o.roots, &l.rel), how);
    }
    let skipped = landed
        .iter()
        .filter(|l| l.outcome == LandOutcome::Kept)
        .map(|l| SkippedPath {
            id: owner_of(&l.rel, &o.lands).map_or_else(|| format!("files/{}", l.rel), |p| p.id.clone()),
            path: landed_at(&o.home, &o.roots, &l.rel),
            note: "already there with other content; wsp did not write over it".to_string(),
        })
        .collect();
    Ok(LandFilesResult { rows, owned, skipped })
}

fn row_of(id: String, label: Option<&str>, dest: &str, rows: &[&Landed], once: bool, home: &str, roots: &StoreRoots) -> PlaceProvisionRow {
    let under = |outcome: LandOutcome| -> Vec<&str> { rows.iter().filter(|r| r.outcome == outcome).map(|r| r.rel.as_str()).collect() };
    let failed = under(LandOutcome::Failed);
    let installed = under(LandOutcome::Installed);
    let kept = under(LandOutcome::Kept);
    let mut notes: Vec<String> = Vec::new();
    if rows.len() > 1 && !installed.is_empty() {
        notes.push(format!("{} of {} files", installed.len(), rows.len()));
    }
    if !kept.is_empty() {
        let head = if kept.len() == rows.len() {
            "already there with other content".to_string()
        } else {
            format!("{} already there with other content", kept.len())
        };
        notes.push(format!("{head}: {}", list_of(&kept)));
    }
    let outcome = if !failed.is_empty() {
        PlaceProvisionRowOutcome::Failed
    } else if !installed.is_empty() {
        PlaceProvisionRowOutcome::Installed
    } else if rows.is_empty() || kept.len() == rows.len() {
        PlaceProvisionRowOutcome::Skipped
    } else {
        PlaceProvisionRowOutcome::Present
    };
    let note = if !failed.is_empty() {
        format!("could not be written: {}", list_of(&failed))
    } else if rows.is_empty() {
        "nothing of it travelled".to_string()
    } else if once && outcome == PlaceProvisionRowOutcome::Present {
        STOOD_LINE.to_string()
    } else {
        notes.join("; ")
    };
    PlaceProvisionRow {
        id,
        label: file_label(label, home, dest, roots),
        outcome,
        kind: Some(PlaceProvisionKind::File),
        step: None,
        note: (!note.is_empty()).then_some(note),
        fix: None,
        keys: None,
        ms: None,
        earlier: None,
        project: None,
    }
}

fn list_of(paths: &[&str]) -> String {
    if paths.len() <= NAMED {
        paths.join(", ")
    } else {
        format!("{} and {} more", paths[..NAMED].join(", "), paths.len() - NAMED)
    }
}

fn last_units(s: &str, n: usize) -> String {
    let units: Vec<u16> = s.encode_utf16().collect();
    String::from_utf16_lossy(&units[units.len().saturating_sub(n)..])
}
