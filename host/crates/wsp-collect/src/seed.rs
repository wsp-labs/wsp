// SPDX-License-Identifier: AGPL-3.0-only
//! The seed menu for one folder on this computer: what a project added from it would carry, read off git's own
//! listing of what it ignores, one size pass and the agent's memory folder. Pure over the Host seam; no file's content
//! is read and nothing leaves the computer. Port of packages/collect/src/seed.ts at a3b870a6e; the spec is spec/seed.md.
use indexmap::IndexMap;
use wsp_catalog::seed::{seed_row_for, SeedKind as CatalogKind, SEED_ROWS};
use wsp_wire::types::js_number::JsInt;
use wsp_wire::views::workspace::{SeedFile, SeedFileRow, SeedKind, SeedPlan, SeedPlanMemory, SeedPlanUnpushed};
use wsp_words::js::{collate, js_trim, to_number};
use wsp_words::project_path::{claude_memory_dir, claude_project_key};

use crate::host::{Host, RunOptions, StatKind};

/// How long one git or du call gets, in ms (seed.ts:13).
pub const CALL_MS: u64 = 120_000;

/// The TS options object `{ claudeStateHome }`.
#[derive(Debug, Clone)]
pub struct SeedOptions {
    pub claude_state_home: String,
}

/// Every path git ignores in the folder, collapsed to the directory where a whole directory is ignored, with the rows
/// the catalog opens broken out, each sized once, plus the branch, the remote, the unpushed commits, the edits that
/// stay here and the agent's memory folder for this folder. `provider` is absent and `remembered` false.
pub async fn seed_menu(host: &Host, folder: &str, o: &SeedOptions) -> SeedPlan {
    let listed = zsplit(git(host, folder, &["ls-files", "-z", "-o", "-i", "--exclude-standard", "--directory"]).await.as_deref());
    let mut opened: IndexMap<String, Vec<String>> = IndexMap::new();
    for entry in &listed {
        if !is_dir(entry) || !opens(entry) {
            continue;
        }
        let inside = zsplit(git(host, folder, &["ls-files", "-z", "-o", "-i", "--exclude-standard", "--", trim(entry)]).await.as_deref());
        opened.insert(entry.clone(), inside.into_iter().filter(|path| seed_row_for(path, false).is_some()).collect());
    }
    let measured: Vec<String> = listed.iter().cloned().chain(opened.values().flatten().cloned()).collect();
    let sizes = du_bytes(host, folder, &measured).await;
    let size = |path: &str| sizes.get(path).copied().unwrap_or(0);
    let mut files: Vec<SeedFile> = Vec::new();
    for entry in &listed {
        let path = trim(entry);
        let broken: &[String] = opened.get(entry).map_or(&[], Vec::as_slice);
        for inner in broken {
            files.extend(row_for(inner, false, size(inner)));
        }
        let own = size(path).saturating_sub(broken.iter().fold(0i64, |sum, inner| sum.saturating_add(size(inner))));
        files.extend(row_for(path, is_dir(entry), own.max(0)));
    }
    files.sort_by(|a, b| collate(&a.path, &b.path));
    let remote = one_line(git(host, folder, &["remote", "get-url", "origin"]).await.as_deref());
    let branch = one_line(git(host, folder, &["rev-parse", "--abbrev-ref", "HEAD"]).await.as_deref()).unwrap_or_default();
    let head = one_line(git(host, folder, &["symbolic-ref", "refs/remotes/origin/HEAD"]).await.as_deref());
    let default_branch = head.map(|h| h.strip_prefix("refs/remotes/origin/").map_or_else(|| h.clone(), str::to_string));
    let unpushed = unpushed(host, folder, default_branch.as_deref()).await;
    let edits = git(host, folder, &["status", "--porcelain"]).await.unwrap_or_default();
    let uncommitted = edits.split('\n').filter(|line| !js_trim(line).is_empty()).count();
    let memory = memory_row(host, folder, &o.claude_state_home).await;
    SeedPlan {
        source: folder.to_string(),
        remote,
        branch,
        default_branch,
        unpushed,
        uncommitted: JsInt::from(uncommitted as i64),
        memory,
        files,
        remembered: false,
        provider: None,
    }
}

/// One git call on the folder, `git -C <folder> <args>`, with the seed's own budget.
async fn git(host: &Host, folder: &str, args: &[&str]) -> Option<String> {
    let mut words = vec!["-C".to_string(), folder.to_string()];
    words.extend(args.iter().map(|arg| arg.to_string()));
    host.exec.run("git", &words, RunOptions { timeout_ms: Some(CALL_MS), ..Default::default() }).await
}

/// The bytes every listed path holds, in one `du -sk` over the whole menu.
async fn du_bytes(host: &Host, folder: &str, paths: &[String]) -> IndexMap<String, i64> {
    let relative: Vec<&str> = paths.iter().map(|path| trim(path)).collect();
    if relative.is_empty() {
        return IndexMap::new();
    }
    let at = format!("{}/", trim(folder));
    let mut args = vec!["-sk".to_string()];
    args.extend(relative.iter().map(|path| format!("{at}{path}")));
    let out = host.exec.run("du", &args, RunOptions { timeout_ms: Some(CALL_MS), ..Default::default() }).await;
    du_sizes(out.as_deref(), folder)
}

async fn memory_row(host: &Host, folder: &str, state_home: &str) -> Option<SeedPlanMemory> {
    let key = claude_project_key(folder);
    let dir = claude_memory_dir(state_home, &key);
    let stat = host.fs.stat(&dir).await?;
    if stat.kind != StatKind::Dir {
        return None;
    }
    let files = host.fs.walk(&dir).await.len();
    Some(SeedPlanMemory { key, files: JsInt::from(files as i64), bytes: JsInt::from(stat.bytes as i64) })
}

/// The commits on this branch the remote does not have, against the upstream or else origin's default branch.
pub async fn unpushed(host: &Host, folder: &str, default_branch: Option<&str>) -> Option<SeedPlanUnpushed> {
    let upstream = one_line(git(host, folder, &["rev-parse", "--abbrev-ref", "@{upstream}"]).await.as_deref());
    let against = match (upstream, default_branch) {
        (Some(upstream), _) => upstream,
        (None, Some(branch)) => format!("origin/{branch}"),
        (None, None) => return None,
    };
    let base = one_line(git(host, folder, &["merge-base", "HEAD", against.as_str()]).await.as_deref())?;
    let range = format!("{base}..HEAD");
    let count_text =
        one_line(git(host, folder, &["rev-list", "--count", range.as_str()]).await.as_deref()).unwrap_or_else(|| "0".to_string());
    let count = to_number(&count_text);
    (count.is_finite() && count.fract() == 0.0 && count > 0.0).then(|| SeedPlanUnpushed { commits: JsInt::from(count as i64), base })
}

/// One row of the menu for a path, or `None` where the catalog calls it junk; ticked for config and secret kinds.
pub fn row_for(path: &str, dir: bool, bytes: i64) -> Option<SeedFile> {
    let (row, kind) = match seed_row_for(path, dir) {
        Some((_, CatalogKind::Junk)) => return None,
        Some((row, kind)) => {
            let kind = match kind {
                CatalogKind::Config => SeedKind::Config,
                CatalogKind::Secret => SeedKind::Secret,
                CatalogKind::Rebuilt => SeedKind::Rebuilt,
                CatalogKind::Data => SeedKind::Data,
                CatalogKind::Never => SeedKind::Never,
                CatalogKind::Junk => SeedKind::Unknown,
            };
            (Some(SeedFileRow { id: row.id.clone(), name: row.name.clone() }), kind)
        }
        None => (None, SeedKind::Unknown),
    };
    let ticked = matches!(kind, SeedKind::Config | SeedKind::Secret);
    Some(SeedFile { path: path.to_string(), dir, bytes: JsInt::from(bytes), kind, row, ticked })
}

/// Whether any catalog row opens this directory entry (`name/` as git lists it).
pub fn opens(entry: &str) -> bool {
    SEED_ROWS.iter().any(|row| row.opens.iter().flatten().any(|name| entry == name || entry.ends_with(&format!("/{name}"))))
}

/// du's `<kibibytes>\t<path>` lines read back against the folder: relative path to bytes (kb × 1024). A line with no
/// tab, a size that is not a number, or a path outside `<folder>/` is skipped.
pub fn du_sizes(out: Option<&str>, folder: &str) -> IndexMap<String, i64> {
    let at = format!("{}/", trim(folder));
    let mut bytes = IndexMap::new();
    for line in out.unwrap_or("").split('\n') {
        let Some(tab) = line.find('\t') else {
            continue;
        };
        let kb = to_number(&line[..tab]);
        let path = trim(&line[tab + 1..]);
        if kb.is_finite() && path.starts_with(&at) {
            bytes.insert(path[at.len()..].to_string(), (kb * 1024.0) as i64);
        }
    }
    bytes
}

fn is_dir(entry: &str) -> bool {
    entry.ends_with('/')
}

fn trim(path: &str) -> &str {
    path.trim_end_matches('/')
}

fn zsplit(out: Option<&str>) -> Vec<String> {
    out.unwrap_or("").split('\0').filter(|word| !word.is_empty()).map(str::to_string).collect()
}

pub fn one_line(out: Option<&str>) -> Option<String> {
    let line = js_trim(out.unwrap_or("").split('\n').next().unwrap_or(""));
    (!line.is_empty()).then(|| line.to_string())
}
