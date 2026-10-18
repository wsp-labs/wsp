// SPDX-License-Identifier: AGPL-3.0-only
//! The tree's rows from the folders listed so far under one root (`components/files/fileTreeRows.ts`): folders first,
//! names in natural order, a note row last under a folder the daemon cut at its cap. A folder shows its children only
//! once opened, and every folder starts closed. A folder that could not list is said beside the tree, never as a row.

use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};

use super::paths::{join, relative};
use super::words::more_entries;

/// One entry of a listing, by the absolute path the daemon reads it at; a symlink is a file, as fs.read follows it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entry {
    pub name: String,
    pub path: String,
    pub dir: bool,
}

/// One folder's listing as the pane holds it (`LevelState`): the last good entries stay through a refresh or a failure.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Level {
    pub entries: Option<Vec<Entry>>,
    /// The daemon cut the folder at its cap; `total` counts what it holds.
    pub truncated: bool,
    pub total: u64,
    pub pending: bool,
    pub error: Option<String>,
}

/// The folders listed so far, by absolute path.
pub type Levels = HashMap<String, Level>;

/// `toProjectEntries` over an fs.list reply.
pub fn level_of(dir: &str, reply: &serde_json::Map<String, serde_json::Value>) -> Level {
    let entries = reply
        .get("entries")
        .and_then(|e| e.as_array())
        .map(|entries| {
            entries
                .iter()
                .filter_map(|e| {
                    let name = e.get("name")?.as_str()?.to_owned();
                    let is_dir = e.get("type").and_then(|t| t.as_str()) == Some("dir");
                    Some(Entry { path: join(dir, &name), name, dir: is_dir })
                })
                .collect()
        })
        .unwrap_or_default();
    Level {
        entries: Some(entries),
        truncated: reply.get("truncated").and_then(|t| t.as_bool()).unwrap_or(false),
        total: reply.get("total").and_then(|t| t.as_u64()).unwrap_or(0),
        pending: false,
        error: None,
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Kind {
    Folder {
        open: bool,
    },
    File,
    /// "… N more entries not shown".
    Note,
}

/// One row of the tree as drawn.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Row {
    /// The entry's absolute path; a note row's is its folder's with a `#more` after it.
    pub path: String,
    pub name: String,
    pub depth: usize,
    pub kind: Kind,
}

/// `localeCompare(.., { numeric: true, sensitivity: "base" })` as far as file names need it: runs of digits by value,
/// the rest without case.
pub fn natural(a: &str, b: &str) -> Ordering {
    let (mut a, mut b) = (a.chars().peekable(), b.chars().peekable());
    loop {
        match (a.peek().copied(), b.peek().copied()) {
            (None, None) => return Ordering::Equal,
            (None, Some(_)) => return Ordering::Less,
            (Some(_), None) => return Ordering::Greater,
            (Some(x), Some(y)) if x.is_ascii_digit() && y.is_ascii_digit() => {
                let run = |it: &mut std::iter::Peekable<std::str::Chars>| {
                    let mut s = String::new();
                    while let Some(c) = it.peek().copied().filter(char::is_ascii_digit) {
                        s.push(c);
                        it.next();
                    }
                    s
                };
                let (x, y) = (run(&mut a), run(&mut b));
                let (tx, ty) = (x.trim_start_matches('0'), y.trim_start_matches('0'));
                let by = tx.len().cmp(&ty.len()).then_with(|| tx.cmp(ty));
                if by != Ordering::Equal {
                    return by;
                }
            }
            (Some(x), Some(y)) => {
                let by = x.to_lowercase().cmp(y.to_lowercase());
                if by != Ordering::Equal {
                    return by;
                }
                a.next();
                b.next();
            }
        }
    }
}

/// `sortTreeRows`: folders first, then files, note rows last, names in natural order.
fn sorted(entries: &[Entry]) -> Vec<&Entry> {
    let mut out: Vec<&Entry> = entries.iter().collect();
    out.sort_by(|l, r| r.dir.cmp(&l.dir).then_with(|| natural(&l.name, &r.name)));
    out
}

/// The rows a person sees: each open folder's children under it. With `search` typed, only the entries whose name holds
/// it and the folders above them, all open, as the tree's hide-non-matches search shows them.
pub fn rows(root: &str, levels: &Levels, open: &HashSet<String>, search: &str) -> Vec<Row> {
    let query = search.trim().to_lowercase();
    let shown = (!query.is_empty()).then(|| matches(root, levels, &query));
    let mut out = Vec::new();
    walk(root, levels, open, shown.as_ref(), 0, &mut out);
    out
}

fn walk(dir: &str, levels: &Levels, open: &HashSet<String>, shown: Option<&HashSet<String>>, depth: usize, out: &mut Vec<Row>) {
    let Some(level) = levels.get(dir) else { return };
    let Some(entries) = &level.entries else { return };
    for entry in sorted(entries) {
        if shown.is_some_and(|s| !s.contains(&entry.path)) {
            continue;
        }
        if entry.dir {
            let is_open = shown.is_some() || open.contains(&entry.path);
            out.push(Row { path: entry.path.clone(), name: entry.name.clone(), depth, kind: Kind::Folder { open: is_open } });
            if is_open {
                walk(&entry.path, levels, open, shown, depth + 1, out);
            }
        } else {
            out.push(Row { path: entry.path.clone(), name: entry.name.clone(), depth, kind: Kind::File });
        }
    }
    if level.truncated && shown.is_none() {
        let hidden = level.total.saturating_sub(entries.len() as u64);
        out.push(Row { path: format!("{dir}#more"), name: more_entries(hidden), depth, kind: Kind::Note });
    }
}

/// Every listed entry whose name holds `query`, with the folders above it up to the root.
fn matches(root: &str, levels: &Levels, query: &str) -> HashSet<String> {
    let mut shown = HashSet::new();
    let mut queue = vec![(root.to_owned(), Vec::<String>::new())];
    while let Some((dir, above)) = queue.pop() {
        let Some(entries) = levels.get(&dir).and_then(|l| l.entries.as_ref()) else { continue };
        for entry in entries {
            if entry.name.to_lowercase().contains(query) {
                shown.insert(entry.path.clone());
                shown.extend(above.iter().cloned());
            }
            if entry.dir {
                let mut next = above.clone();
                next.push(entry.path.clone());
                queue.push((entry.path.clone(), next));
            }
        }
    }
    shown
}

/// The folders under the root whose listing the tree holds, the root first: what a refresh asks for again.
pub fn loaded(root: &str, levels: &Levels) -> Vec<String> {
    reached(root, levels).into_iter().filter(|dir| levels.get(dir).is_some_and(|l| l.entries.is_some())).collect()
}

/// Folders under the root that failed to list, by their path below it, with the daemon's word.
pub fn errors(root: &str, levels: &Levels) -> Vec<(String, String)> {
    reached(root, levels)
        .into_iter()
        .filter(|dir| dir != root)
        .filter_map(|dir| {
            let level = levels.get(&dir)?;
            match (&level.entries, &level.error) {
                (None, Some(error)) => Some((relative(root, &dir).to_owned(), error.clone())),
                _ => None,
            }
        })
        .collect()
}

/// The root and every folder below it that has a level, breadth first, as `fileTreeRows` walks them.
fn reached(root: &str, levels: &Levels) -> Vec<String> {
    let mut queue = vec![root.to_owned()];
    let mut ix = 0;
    while ix < queue.len() {
        let dir = queue[ix].clone();
        ix += 1;
        let Some(entries) = levels.get(&dir).and_then(|l| l.entries.as_ref()) else { continue };
        queue.extend(entries.iter().filter(|e| e.dir && levels.contains_key(&e.path)).map(|e| e.path.clone()));
    }
    queue.retain(|dir| levels.contains_key(dir));
    queue
}
