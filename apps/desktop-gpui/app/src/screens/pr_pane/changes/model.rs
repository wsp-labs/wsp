// SPDX-License-Identifier: AGPL-3.0-only
//! What the Changes pane decides without drawing: the scopes and their words (`diffs/model.ts`), what it keeps on the
//! local store under the web's key (`diffs/store.ts`), the changed-files tree (`lib/turnDiffTree.ts`), which files show
//! folded and viewed, and what each file allows.

use std::collections::{BTreeMap, HashMap, HashSet};

use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};

use crate::local_store::LocalStore;
use crate::screens::pr_pane::diff::{FilePatch, FileStatus, Layout};
pub use crate::screens::pr_pane::files::paths::{base_name, relative as relative_to};
use crate::screens::pr_pane::files::rows::natural;

/// The web's persisted diff surface (`useDiffStore`'s `persist` name).
pub const STORE_KEY: &str = "wsp:diff-surface:v2";
/// `KEPT_TURNS`: the turns kept on this computer, oldest dropped first.
pub const KEPT_TURNS: usize = 100;

/// The scope menu's picks, in its order.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Scope {
    /// The branch against its merge-base, the default: the one scope that holds what a thread changed whether it
    /// committed or not.
    #[default]
    Branch,
    Head,
    Unstaged,
    Staged,
}

impl Scope {
    pub const ALL: [Scope; 4] = [Scope::Branch, Scope::Head, Scope::Unstaged, Scope::Staged];

    pub fn label(self) -> &'static str {
        match self {
            Scope::Head => "Uncommitted",
            Scope::Unstaged => "Working tree",
            Scope::Staged => "Staged",
            Scope::Branch => "Branch changes",
        }
    }

    /// What the scope holds, as a sentence names it: "No uncommitted changes at /root/app."
    pub fn noun(self) -> &'static str {
        match self {
            Scope::Head => "uncommitted changes",
            Scope::Unstaged => "working tree changes",
            Scope::Staged => "staged changes",
            Scope::Branch => "branch changes",
        }
    }

    /// The daemon's `GitDiffScope` word.
    pub fn wire(self) -> &'static str {
        match self {
            Scope::Branch => "branch",
            Scope::Head => "head",
            Scope::Unstaged => "unstaged",
            Scope::Staged => "staged",
        }
    }
}

/// One turn's changes, as a reply's changed-files card opens them: the snapshots at its launch and its end, the
/// folder they were taken in, the file asked for, and the files of one list of the card it is narrowed to.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TurnRange {
    pub thread_id: String,
    pub turn_id: String,
    pub cwd: String,
    pub from: String,
    pub to: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub only: Option<Vec<String>>,
}

impl TurnRange {
    /// The same range with nothing narrowed: Show all.
    pub fn whole(&self) -> TurnRange {
        TurnRange { only: None, ..self.clone() }
    }
}

/// What the pane keeps between runs, in the web's shape: `{ state: { scopeByWorkspaceId, turnByThread, renderMode },
/// version: 0 }`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Kept {
    pub scopes: BTreeMap<String, Scope>,
    /// By `<workspace id>#<thread id>`, oldest first, at most [`KEPT_TURNS`].
    pub turns: Vec<(String, TurnRange)>,
    pub layout: Layout,
}

pub fn turn_key(workspace_id: &str, thread_id: &str) -> String {
    format!("{workspace_id}#{thread_id}")
}

impl Kept {
    pub fn read(store: &LocalStore) -> Kept {
        let state = store.get(STORE_KEY).and_then(|v| v.get("state")).cloned().unwrap_or(Value::Null);
        let scopes = state
            .get("scopeByWorkspaceId")
            .and_then(|v| serde_json::from_value::<BTreeMap<String, Value>>(v.clone()).ok())
            .unwrap_or_default()
            .into_iter()
            .filter_map(|(id, scope)| serde_json::from_value(scope).ok().map(|s| (id, s)))
            .collect();
        let turns = state
            .get("turnByThread")
            .and_then(Value::as_object)
            .map(|turns| turns.iter().filter_map(|(k, v)| serde_json::from_value(v.clone()).ok().map(|t| (k.clone(), t))).collect())
            .unwrap_or_default();
        let layout = if state.get("renderMode").and_then(Value::as_str) == Some("split") { Layout::Split } else { Layout::Stacked };
        Kept { scopes, turns, layout }
    }

    pub fn write(&self, store: &mut LocalStore) -> std::io::Result<()> {
        let turns: Map<String, Value> = self.turns.iter().map(|(k, t)| (k.clone(), json!(t))).collect();
        let mode = if self.layout == Layout::Split { "split" } else { "stacked" };
        store.set(
            STORE_KEY,
            json!({ "state": { "scopeByWorkspaceId": self.scopes, "turnByThread": turns, "renderMode": mode }, "version": 0 }),
        )
    }

    pub fn scope(&self, workspace_id: &str) -> Scope {
        self.scopes.get(workspace_id).copied().unwrap_or_default()
    }

    pub fn turn(&self, workspace_id: &str, thread_id: &str) -> Option<&TurnRange> {
        let key = turn_key(workspace_id, thread_id);
        self.turns.iter().find(|(k, _)| *k == key).map(|(_, t)| t)
    }

    /// Takes the workspace's scope and drops the turn the thread on screen had open.
    pub fn set_scope(&mut self, workspace_id: &str, thread_id: Option<&str>, scope: Scope) {
        self.scopes.insert(workspace_id.to_owned(), scope);
        if let Some(thread) = thread_id {
            let key = turn_key(workspace_id, thread);
            self.turns.retain(|(k, _)| *k != key);
        }
    }

    /// Opens a turn's range for its thread; written last goes last, so the turns past the cap are the oldest opened.
    pub fn open_turn(&mut self, workspace_id: &str, range: TurnRange) {
        let key = turn_key(workspace_id, &range.thread_id);
        self.turns.retain(|(k, _)| *k != key);
        self.turns.push((key, range));
        let over = self.turns.len().saturating_sub(KEPT_TURNS);
        self.turns.drain(..over);
    }
}

/// Keeps a turn's range for its thread on the local store, so the Changes pane opens on it (the turn card's Open
/// diff). The thread view calls this, then opens the pane.
pub fn open_turn(store: &mut LocalStore, workspace_id: &str, range: TurnRange) -> std::io::Result<()> {
    let mut kept = Kept::read(store);
    kept.open_turn(workspace_id, range);
    kept.write(store)
}

/// `onlyOf`: the files a turn's range opens narrowed to, where the turn's own files are known apart from the folder's
/// others: the others' when the file asked for is theirs or the turn has none of its own, else its own.
pub fn only_of(files: &[String], others: Option<&[String]>, path: Option<&str>) -> Option<Vec<String>> {
    let others = others?;
    Some(if others.iter().any(|f| Some(f.as_str()) == path) || files.is_empty() { others.to_vec() } else { files.to_vec() })
}

/// `changedPathOf`: the path a turn's changes name a file by, from the top of the checkout, for a path an edit named
/// whole; the longest match wins, as git lists `package.json` before `packages/x/package.json`.
pub fn changed_path_of(paths: &[String], named: &str) -> Option<String> {
    paths.iter().filter(|p| named == p.as_str() || named.ends_with(&format!("/{p}"))).max_by_key(|p| p.len()).cloned()
}

/// What one file allows here: an edit where its new side is the file itself and its patch is whole text,
/// a discard outside Branch changes and a turn.
pub fn editable(file: &FilePatch, scope: Scope, turn: bool) -> bool {
    !turn && scope != Scope::Staged && file.whole_text()
}

pub fn discardable(scope: Scope, turn: bool) -> bool {
    !turn && scope != Scope::Branch
}

/// A commit takes what Uncommitted lists, and nothing from a turn.
pub fn committable(scope: Scope, turn: bool, files: usize) -> bool {
    !turn && scope == Scope::Head && files > 0
}

/// A new file in a scope measured against HEAD is one no commit has, so a discard deletes it.
pub fn discard_deletes(file: &FilePatch, scope: Scope) -> bool {
    matches!(scope, Scope::Head | Scope::Staged) && file.status == FileStatus::Added
}

/// Whether the file's mark names the contents it has now.
pub fn is_viewed(file: &FilePatch, marks: &HashMap<String, String>) -> bool {
    file.blob.as_ref().is_some_and(|blob| marks.get(file.path.as_ref()) == Some(blob))
}

/// Which files show folded: the ones a person folded, and the viewed ones nobody opened again since.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Folds {
    pub collapsed: HashSet<String>,
    pub opened: HashSet<String>,
}

impl Folds {
    pub fn folded(&self, path: &str, viewed: bool) -> bool {
        self.collapsed.contains(path) || (viewed && !self.opened.contains(path))
    }

    pub fn toggle(&mut self, path: &str, viewed: bool) {
        let shut = self.folded(path, viewed);
        if shut {
            self.collapsed.remove(path);
            self.opened.insert(path.to_owned());
        } else {
            self.collapsed.insert(path.to_owned());
            self.opened.remove(path);
        }
    }

    /// A file marked viewed folds again even where it was opened.
    pub fn marked(&mut self, path: &str) {
        self.opened.remove(path);
    }

    /// Fold all, or open all where every file is folded already (`toggleAllDiffFiles`).
    pub fn toggle_all<'a>(&mut self, paths: impl IntoIterator<Item = &'a str> + Clone) -> bool {
        let all = paths.clone().into_iter().all(|p| self.collapsed.contains(p));
        if all {
            for p in paths {
                self.collapsed.remove(p);
            }
        } else {
            self.collapsed.extend(paths.into_iter().map(str::to_owned));
        }
        !all
    }

    pub fn all_collapsed<'a>(&self, paths: impl IntoIterator<Item = &'a str>) -> bool {
        let mut any = false;
        for p in paths {
            any = true;
            if !self.collapsed.contains(p) {
                return false;
            }
        }
        any
    }
}

/// One file of the changed-files tree.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TreeFile {
    pub path: String,
    pub additions: usize,
    pub deletions: usize,
}

/// A row of the changed-files tree as drawn: a folder (its chain of lone folders joined, `a/b/c`) or a file, at a
/// depth, with its counts.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TreeRow {
    pub depth: usize,
    pub name: String,
    pub path: String,
    pub folder: bool,
    pub additions: usize,
    pub deletions: usize,
}

#[derive(Default)]
struct Dir {
    name: String,
    path: String,
    adds: usize,
    dels: usize,
    dirs: BTreeMap<String, Dir>,
    files: Vec<TreeFile>,
}

/// The tree's rows with every folder open but the ones in `shut`, folders first in natural order, then files.
pub fn tree_rows(files: &[TreeFile], shut: &HashSet<String>) -> Vec<TreeRow> {
    let mut root = Dir::default();
    for file in files {
        let parts: Vec<&str> = file.path.split(['/', '\\']).filter(|p| !p.is_empty()).collect();
        let Some((_, dirs)) = parts.split_last() else { continue };
        let mut at = &mut root;
        let mut path = String::new();
        for part in dirs {
            path = if path.is_empty() { (*part).to_owned() } else { format!("{path}/{part}") };
            let next =
                at.dirs.entry((*part).to_owned()).or_insert_with(|| Dir { name: (*part).to_owned(), path: path.clone(), ..Dir::default() });
            next.adds += file.additions;
            next.dels += file.deletions;
            at = next;
        }
        at.files.push(file.clone());
    }
    let mut rows = Vec::new();
    push_rows(&root, 0, shut, &mut rows);
    rows
}

fn push_rows(dir: &Dir, depth: usize, shut: &HashSet<String>, rows: &mut Vec<TreeRow>) {
    let mut dirs: Vec<&Dir> = dir.dirs.values().collect();
    dirs.sort_by(|a, b| natural(&a.name, &b.name));
    for sub in dirs {
        let (mut shown, mut name) = (sub, sub.name.clone());
        while shown.files.is_empty() && shown.dirs.len() == 1 {
            let only = shown.dirs.values().next().expect("one folder");
            name = format!("{name}/{}", only.name);
            shown = only;
        }
        rows.push(TreeRow { depth, name, path: shown.path.clone(), folder: true, additions: shown.adds, deletions: shown.dels });
        if !shut.contains(&shown.path) {
            push_rows(shown, depth + 1, shut, rows);
        }
    }
    let mut files: Vec<&TreeFile> = dir.files.iter().collect();
    files.sort_by(|a, b| natural(base_name(&a.path), base_name(&b.path)));
    for file in files {
        rows.push(TreeRow {
            depth,
            name: base_name(&file.path).to_owned(),
            path: file.path.clone(),
            folder: false,
            additions: file.additions,
            deletions: file.deletions,
        });
    }
}

/// `formatCompactDiffCount`: 999, 1.2k, 34k, 1.5m.
pub fn compact(n: usize) -> String {
    let unit = |v: f64, suffix: &str| {
        if v < 10. {
            let s = format!("{v:.1}");
            format!("{}{suffix}", s.strip_suffix(".0").unwrap_or(&s))
        } else {
            format!("{}{suffix}", v.round())
        }
    };
    match n {
        0..=999 => n.to_string(),
        1_000..=999_999 => unit(n as f64 / 1e3, "k"),
        1_000_000..=999_999_999 => unit(n as f64 / 1e6, "m"),
        _ => unit(n as f64 / 1e9, "b"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn file(path: &str, a: usize, d: usize) -> TreeFile {
        TreeFile { path: path.into(), additions: a, deletions: d }
    }

    #[test]
    fn the_tree_puts_folders_first_joins_lone_folders_and_sums_counts() {
        let files = [file("src/keep.ts", 2, 1), file("logo.bin", 0, 0), file("src/added.ts", 1, 0), file("a/b/c/deep.ts", 3, 0)];
        let rows = tree_rows(&files, &HashSet::new());
        let shown: Vec<(usize, &str, bool)> = rows.iter().map(|r| (r.depth, r.name.as_str(), r.folder)).collect();
        assert_eq!(
            shown,
            [
                (0, "a/b/c", true),
                (1, "deep.ts", false),
                (0, "src", true),
                (1, "added.ts", false),
                (1, "keep.ts", false),
                (0, "logo.bin", false)
            ]
        );
        assert_eq!((rows[2].additions, rows[2].deletions), (3, 1));
        let shut: HashSet<String> = ["src".to_owned()].into();
        assert_eq!(tree_rows(&files, &shut).len(), 4);
    }

    #[test]
    fn a_turns_range_narrows_to_the_list_its_file_is_in_and_names_files_from_the_top() {
        let (own, others) = (vec!["src/a.rs".to_owned()], vec!["package.json".to_owned(), "packages/x/package.json".to_owned()]);
        assert_eq!(only_of(&own, None, Some("src/a.rs")), None, "no others leaves the range whole");
        assert_eq!(only_of(&own, Some(&others), Some("src/a.rs")), Some(own.clone()));
        assert_eq!(only_of(&own, Some(&others), Some("package.json")), Some(others.clone()));
        assert_eq!(only_of(&[], Some(&others), None), Some(others.clone()), "a turn with none of its own opens the others");
        let all: Vec<String> = own.iter().chain(&others).cloned().collect();
        assert_eq!(changed_path_of(&all, "/r/packages/x/package.json").as_deref(), Some("packages/x/package.json"));
        assert_eq!(changed_path_of(&all, "src/a.rs").as_deref(), Some("src/a.rs"));
        assert_eq!(changed_path_of(&all, "/r/src/b.rs"), None);
    }

    #[test]
    fn counts_compact_as_the_web_writes_them() {
        assert_eq!([compact(999), compact(1200), compact(34_400), compact(1_500_000)], ["999", "1.2k", "34k", "1.5m"]);
    }

    #[test]
    fn a_viewed_file_folds_until_opened_and_a_mark_folds_it_again() {
        let mut folds = Folds::default();
        assert!(folds.folded("a", true) && !folds.folded("a", false));
        folds.toggle("a", true);
        assert!(!folds.folded("a", true));
        folds.marked("a");
        assert!(folds.folded("a", true));
        assert!(folds.toggle_all(["a", "b"]));
        assert!(folds.all_collapsed(["a", "b"]));
        assert!(!folds.toggle_all(["a", "b"]));
        assert!(!folds.folded("b", false));
    }

    #[test]
    fn the_kept_state_reads_back_in_the_web_shape_and_caps_turns() {
        let dir = tempfile::tempdir().unwrap();
        let mut store = LocalStore::open(dir.path().join("s.json"));
        let mut kept = Kept::read(&store);
        assert_eq!(kept.scope("w"), Scope::Branch);
        kept.set_scope("w", Some("t"), Scope::Head);
        kept.layout = Layout::Split;
        for i in 0..(KEPT_TURNS + 3) {
            let range = TurnRange {
                thread_id: format!("t{i}"),
                turn_id: "u".into(),
                cwd: "/r".into(),
                from: "a".into(),
                to: "b".into(),
                path: None,
                only: None,
            };
            kept.open_turn("w", range);
        }
        kept.write(&mut store).unwrap();
        let back = Kept::read(&LocalStore::open(dir.path().join("s.json")));
        assert_eq!(back.scope("w"), Scope::Head);
        assert_eq!(back.layout, Layout::Split);
        assert_eq!(back.turns.len(), KEPT_TURNS);
        assert!(back.turn("w", "t0").is_none() && back.turn("w", "t102").is_some());
        let raw = store.get(STORE_KEY).unwrap();
        assert_eq!(raw["state"]["scopeByWorkspaceId"]["w"], "head");
        assert_eq!(raw["state"]["renderMode"], "split");
    }
}
