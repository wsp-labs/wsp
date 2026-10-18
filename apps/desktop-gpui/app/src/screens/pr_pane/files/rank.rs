// SPDX-License-Identifier: AGPL-3.0-only
//! The finder's ranking (`files/fileFinder.logic.ts` over `files/searchRanking.ts`, after t3code's, MIT): the daemon
//! answers every path whose letters hold the query in order and ranks none, so the name is ranked here before its
//! folders, by tiers: exact, prefix, at a word's edge, anywhere, then letters in order. Lower scores rank first.

/// `FILE_FINDER_RESULT_LIMIT`.
pub const LIMIT: usize = 50;

/// One file the finder lists: its path as the daemon names it, relative to the folder searched, its name, and the
/// folders above it, "" at the top.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FileMatch {
    pub path: String,
    pub name: String,
    pub folder: String,
}

/// `fileFinderQuery`: lowercased, no leading `@`, `.` or `/`, no spaces.
pub fn file_query(raw: &str) -> String {
    raw.trim().trim_start_matches(['@', '.', '/']).to_lowercase().chars().filter(|c| !c.is_whitespace()).collect()
}

fn match_of(path: &str) -> FileMatch {
    match path.rfind('/') {
        Some(cut) => FileMatch { path: path.to_owned(), name: path[cut + 1..].to_owned(), folder: path[..cut].to_owned() },
        None => FileMatch { path: path.to_owned(), name: path.to_owned(), folder: String::new() },
    }
}

fn length_penalty(value: &[char], query: &[char]) -> i64 {
    (value.len() as i64 - query.len() as i64).clamp(0, 64)
}

/// `scoreSubsequenceMatch`.
fn subsequence(value: &[char], query: &[char]) -> Option<i64> {
    if query.is_empty() {
        return Some(0);
    }
    let (mut qi, mut first, mut prev, mut gaps) = (0usize, None::<usize>, None::<usize>, 0i64);
    for (vi, c) in value.iter().enumerate() {
        if *c != query[qi] {
            continue;
        }
        let start = *first.get_or_insert(vi);
        if let Some(p) = prev {
            gaps += (vi - p - 1) as i64;
        }
        prev = Some(vi);
        qi += 1;
        if qi == query.len() {
            let span = (vi - start + 1 - query.len()) as i64;
            let length = (value.len() as i64 - query.len() as i64).min(64);
            return Some(start as i64 * 2 + gaps * 3 + span + length);
        }
    }
    None
}

fn find(value: &[char], needle: &[char]) -> Option<usize> {
    if needle.is_empty() || needle.len() > value.len() {
        return None;
    }
    (0..=value.len() - needle.len()).find(|&i| value[i..i + needle.len()] == *needle)
}

struct Bases {
    exact: i64,
    prefix: i64,
    boundary: i64,
    includes: i64,
    fuzzy: i64,
}

/// `scoreQueryMatch` with every tier on and the boundary marks space, `-`, `_` and `/`.
fn score(value: &str, query: &str, b: &Bases) -> Option<i64> {
    if value.is_empty() || query.is_empty() {
        return None;
    }
    if value == query {
        return Some(b.exact);
    }
    let (v, q): (Vec<char>, Vec<char>) = (value.chars().collect(), query.chars().collect());
    if value.starts_with(query) {
        return Some(b.prefix + length_penalty(&v, &q));
    }
    let boundary = [' ', '-', '_', '/']
        .iter()
        .filter_map(|mark| {
            let mut needle = vec![*mark];
            needle.extend(&q);
            find(&v, &needle).map(|ix| ix + 1)
        })
        .min();
    if let Some(ix) = boundary {
        return Some(b.boundary + ix as i64 * 2 + length_penalty(&v, &q));
    }
    if let Some(ix) = find(&v, &q) {
        return Some(b.includes + ix as i64 * 2 + length_penalty(&v, &q));
    }
    subsequence(&v, &q).map(|s| b.fuzzy + s)
}

const BY_NAME: Bases = Bases { exact: 0, prefix: 10, boundary: 20, includes: 30, fuzzy: 100 };
const BY_PATH: Bases = Bases { exact: 5, prefix: 40, boundary: 50, includes: 60, fuzzy: 200 };

/// `rankFilePaths`: the best `limit` paths for the query, a name match before a match in the folders, ties by path;
/// with no query, the first `limit` in the daemon's order.
pub fn rank(paths: &[String], raw: &str, limit: usize) -> Vec<FileMatch> {
    let query = file_query(raw);
    if query.is_empty() {
        return paths.iter().take(limit).map(|p| match_of(p)).collect();
    }
    let mut ranked: Vec<(i64, FileMatch)> = paths
        .iter()
        .filter_map(|path| {
            let m = match_of(path);
            let by_name = score(&m.name.to_lowercase(), &query, &BY_NAME);
            let by_path = score(&path.to_lowercase(), &query, &BY_PATH);
            by_name.into_iter().chain(by_path).min().map(|s| (s, m))
        })
        .collect();
    ranked.sort_by(|(a, l), (b, r)| a.cmp(b).then_with(|| l.path.cmp(&r.path)));
    ranked.truncate(limit);
    ranked.into_iter().map(|(_, m)| m).collect()
}
