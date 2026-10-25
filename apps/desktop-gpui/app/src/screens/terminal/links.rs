// SPDX-License-Identifier: AGPL-3.0-only
//! Web addresses and paths in a terminal's text, as today's pane finds them (`terminal-links.ts`,
//! `terminal/ghostty/surface/links.ts`): over the soft-wrapped line through a cell, addresses first, then paths that
//! do not overlap one. Offsets count characters.

use std::sync::OnceLock;

use regex::Regex;

use super::engine::Row;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Url,
    Path,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Match {
    pub kind: Kind,
    pub text: String,
    pub start: usize,
    pub end: usize,
}

/// A cell of the viewport, by column and row.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct At {
    pub x: u16,
    pub y: u16,
}

/// A link on screen: its text and the cells it covers, both ends inclusive.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Found {
    pub kind: Kind,
    pub text: String,
    pub start: At,
    pub end: At,
}

impl Found {
    pub fn covers(&self, x: u16, y: u16) -> bool {
        (self.start.y, self.start.x) <= (y, x) && (y, x) <= (self.end.y, self.end.x)
    }
}

fn url_pattern() -> &'static Regex {
    static P: OnceLock<Regex> = OnceLock::new();
    P.get_or_init(|| Regex::new(r#"(?i)https?://[^\s"'`<>]+"#).expect("the address pattern compiles"))
}

fn path_pattern() -> &'static Regex {
    static P: OnceLock<Regex> = OnceLock::new();
    P.get_or_init(|| {
        Regex::new(r#"(?:~/|\.{1,2}/|/|[A-Za-z]:[\\/]|\\\\)[^\s"'`<>]+|[A-Za-z0-9._-]+(?:/[A-Za-z0-9._-]+)+(?::\d+){0,2}"#)
            .expect("the path pattern compiles")
    })
}

pub fn is_url(text: &str) -> bool {
    let starts = |head: &str| text.get(..head.len()).is_some_and(|h| h.eq_ignore_ascii_case(head));
    starts("http://") || starts("https://")
}

fn trim_closing(raw: &str) -> &str {
    let mut out = raw.trim_end_matches(['.', ',', ';', '!', '?']);
    for (open, close) in [('(', ')'), ('[', ']'), ('{', '}')] {
        while out.ends_with(close) && out.matches(open).count() < out.matches(close).count() {
            out = &out[..out.len() - 1];
        }
    }
    out
}

fn collect(line: &str, kind: Kind, pattern: &Regex, existing: &[Match], out: &mut Vec<Match>) {
    let first = out.len();
    for m in pattern.find_iter(line) {
        let text = trim_closing(m.as_str());
        if text.is_empty() || (kind == Kind::Path && is_url(text)) {
            continue;
        }
        let start = line[..m.start()].chars().count();
        let end = start + text.chars().count();
        let collides = existing.iter().chain(&out[first..]).any(|o| start < o.end && o.start < end);
        if !collides {
            out.push(Match { kind, text: text.to_owned(), start, end });
        }
    }
}

/// `extractTerminalLinks`: every address and path in `line`, in order.
pub fn extract(line: &str) -> Vec<Match> {
    let mut urls = Vec::new();
    collect(line, Kind::Url, url_pattern(), &[], &mut urls);
    let mut all = urls.clone();
    collect(line, Kind::Path, path_pattern(), &urls, &mut all);
    all.sort_by_key(|m| m.start);
    all
}

/// A row's text as the link finder reads it: each cell's text, a blank cell a space, a wide character's tail nothing.
fn row_text(row: &Row) -> String {
    row.cells.iter().filter(|c| c.width > 0).map(|c| if c.len == 0 { " " } else { row.cell_text(c) }).collect()
}

/// The character offset of `column` in `row_text(row)`, and back.
fn column_offset(row: &Row, column: u16) -> usize {
    row.cells
        .iter()
        .take(column as usize)
        .filter(|c| c.width > 0)
        .map(|c| if c.len == 0 { 1 } else { row.cell_text(c).chars().count() })
        .sum()
}

fn column_at(row: &Row, offset: usize) -> u16 {
    for column in 0..row.cells.len() as u16 {
        if offset < column_offset(row, column + 1) {
            return column;
        }
    }
    row.cells.len().saturating_sub(1) as u16
}

/// Whether row `y` carries on the row above it. A scroll region can mark a row as continuing one not marked as wrapping.
fn joins_above(rows: &[Row], y: usize) -> bool {
    y > 0 && rows[y].wrap_continuation && rows[y - 1].wraps
}

/// `terminalLinkAtPositionWithRange`: the link under the cell (`x`, `y`) of the viewport's rows, if any.
pub fn at(rows: &[Row], y: usize, x: u16) -> Option<Found> {
    rows.get(y)?;
    let mut head = y;
    while joins_above(rows, head) {
        head -= 1;
    }
    // A line whose head scrolled above the viewport, or out of a scroll region, would read as a truncated link.
    if rows[head].wrap_continuation {
        return None;
    }
    let mut tail = head;
    while tail + 1 < rows.len() && joins_above(rows, tail + 1) {
        tail += 1;
    }
    let mut text = String::new();
    let mut starts = Vec::new();
    for (i, row) in rows[head..=tail].iter().enumerate() {
        let mut t = row_text(row);
        if head + i == tail {
            t.truncate(t.trim_end().len());
        }
        starts.push(text.chars().count());
        text.push_str(&t);
    }
    let total = text.chars().count();
    let continues_below = rows[tail].wraps;
    let offset = starts[y - head] + column_offset(&rows[y], x);
    let segment_of = |offset: usize| (0..starts.len()).rev().find(|&i| starts[i] <= offset).unwrap_or(0);
    for m in extract(&text) {
        if offset < m.start || offset >= m.end {
            continue;
        }
        if m.end == total && continues_below {
            return None;
        }
        let (s, e) = (segment_of(m.start), segment_of(m.end - 1));
        let start = At { x: column_at(&rows[head + s], m.start - starts[s]), y: (head + s) as u16 };
        let end = At { x: column_at(&rows[head + e], m.end - 1 - starts[e]), y: (head + e) as u16 };
        return Some(Found { kind: m.kind, text: m.text, start, end });
    }
    None
}

fn home_from(cwd: &str) -> Option<String> {
    for prefix in ["/Users/", "/home/"] {
        if let Some(rest) = cwd.strip_prefix(prefix) {
            let user = rest.split('/').next().filter(|u| !u.is_empty())?;
            return Some(format!("{prefix}{user}"));
        }
    }
    let b = cwd.as_bytes();
    if b.len() > 9 && b[0].is_ascii_alphabetic() && cwd[1..].starts_with(":\\Users\\") {
        let user = cwd[9..].split('\\').next().filter(|u| !u.is_empty())?;
        return Some(format!("{}{user}", &cwd[..9]));
    }
    None
}

fn windows_absolute(p: &str) -> bool {
    let b = p.as_bytes();
    (b.len() >= 3 && b[0].is_ascii_alphabetic() && b[1] == b':' && (b[2] == b'\\' || b[2] == b'/')) || p.starts_with("\\\\")
}

fn windows_style(p: &str) -> bool {
    windows_absolute(p) || p.as_bytes().windows(3).any(|w| w[0].is_ascii_alphabetic() && w[1] == b':' && w[2] == b'\\')
}

fn join(base: &str, next: &str, windows: bool) -> String {
    let base = base.trim_end_matches(['/', '\\']);
    if windows {
        format!("{base}\\{}", next.replace('/', "\\"))
    } else {
        format!("{base}/{}", next.trim_start_matches('/'))
    }
}

/// Splits a trailing `:line` or `:line:column` off a path.
fn split_position(value: &str) -> (&str, Option<&str>, Option<&str>) {
    fn tail_num(s: &str) -> Option<(&str, &str)> {
        let (head, num) = s.rsplit_once(':')?;
        (!num.is_empty() && num.bytes().all(|b| b.is_ascii_digit())).then_some((head, num))
    }
    let Some((path, last)) = tail_num(value) else { return (value, None, None) };
    match tail_num(path) {
        Some((path, line)) => (path, Some(line), Some(last)),
        None => (path, Some(last), None),
    }
}

/// `walked` in today's `files/open.ts`: an absolute path with its `.` and `..` segments walked.
fn walked(path: &str) -> String {
    let mut parts = Vec::new();
    for part in path.split('/') {
        match part {
            "" | "." => {}
            ".." => _ = parts.pop(),
            _ => parts.push(part),
        }
    }
    format!("/{}", parts.join("/"))
}

/// `resolvePathLinkTarget` then `walked`: a path as the Files pane opens it, against `cwd`, its `:line:column` kept.
pub fn resolve_path(raw: &str, cwd: &str) -> String {
    let (path, line, column) = split_position(raw);
    let resolved = if let Some(rest) = path.strip_prefix("~/") {
        match home_from(cwd) {
            Some(home) => {
                let w = windows_style(&home);
                join(&home, rest, w)
            }
            None => path.to_owned(),
        }
    } else if !(path.starts_with('/') || windows_absolute(path)) {
        join(cwd, path, windows_style(cwd))
    } else {
        path.to_owned()
    };
    // Walking splits on `/` and roots the result there, so it would break a Windows or unresolved `~` path.
    let resolved = if resolved.starts_with('/') { walked(&resolved) } else { resolved };
    match (line, column) {
        (Some(l), Some(c)) => format!("{resolved}:{l}:{c}"),
        (Some(l), None) => format!("{resolved}:{l}"),
        _ => resolved,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::screens::terminal::engine::Engine;

    fn m(kind: Kind, text: &str, start: usize) -> Match {
        Match { kind, text: text.into(), start, end: start + text.chars().count() }
    }

    #[test]
    fn finds_addresses_and_paths() {
        let line = "failed at https://example.com/docs and src/components/ThreadTerminalDrawer.tsx:42";
        assert_eq!(
            extract(line),
            [m(Kind::Url, "https://example.com/docs", 10), m(Kind::Path, "src/components/ThreadTerminalDrawer.tsx:42", 39)]
        );
        assert_eq!(extract("open HTTPS://example.com/docs"), [m(Kind::Url, "HTTPS://example.com/docs", 5)]);
        assert!(is_url("Http://example.com") && !is_url("httpsdocs/readme.md"));
    }

    #[test]
    fn trims_trailing_punctuation_and_unbalanced_closers() {
        let line = "(https://example.com/docs), ./src/main.ts:12.";
        assert_eq!(extract(line), [m(Kind::Url, "https://example.com/docs", 1), m(Kind::Path, "./src/main.ts:12", 28)]);
        assert_eq!(extract("(C:/tmp/x.ts)."), [m(Kind::Path, "C:/tmp/x.ts", 1)]);
        let line = "see C:/Users/someone/project/src/file.ts:42 for details";
        assert_eq!(extract(line), [m(Kind::Path, "C:/Users/someone/project/src/file.ts:42", 4)]);
    }

    #[test]
    fn resolves_paths_against_the_shells_folder() {
        assert_eq!(
            resolve_path("src/components/ThreadTerminalDrawer.tsx:42:7", "/Users/julius/project"),
            "/Users/julius/project/src/components/ThreadTerminalDrawer.tsx:42:7"
        );
        assert_eq!(resolve_path("/Users/julius/project/src/main.ts:12", "/Users/julius/project"), "/Users/julius/project/src/main.ts:12");
        assert_eq!(
            resolve_path("C:/Users/julius/project/src/main.ts:12", "C:\\Users\\julius\\project"),
            "C:/Users/julius/project/src/main.ts:12"
        );
        assert_eq!(resolve_path("~/notes.md", "/home/ana/work"), "/home/ana/notes.md");
    }

    #[test]
    fn a_resolved_path_walks_its_dots_and_keeps_its_line_and_column() {
        assert_eq!(resolve_path("../lib/./x.rs:3:9", "/home/ana/w x/src"), "/home/ana/w x/lib/x.rs:3:9");
        assert_eq!(resolve_path("./a//b/../c.ts:12", "/repo/"), "/repo/a/c.ts:12");
        assert_eq!(resolve_path("/etc/../tmp/./log", "/repo"), "/tmp/log");
        assert_eq!(resolve_path("../../../../x", "/a"), "/x", "`..` stops at the root");
    }

    fn rows_of(cols: u16, rows: u16, bytes: &[u8]) -> Vec<Row> {
        let mut e = Engine::new(cols, rows);
        e.write(bytes);
        let mut frame = Default::default();
        e.frame(&mut frame);
        frame.rows
    }

    #[test]
    fn paths_and_addresses_among_wide_text_are_read_without_cutting_a_character() {
        assert!(!is_url("~/日本語/x"));
        assert!(!is_url("日本語"));
        let found = extract("見て ~/日本語/x と https://example.com/日本");
        assert_eq!(found, [m(Kind::Path, "~/日本語/x", 3), m(Kind::Url, "https://example.com/日本", 13)]);
    }

    #[test]
    fn a_link_soft_wrapped_over_two_rows_is_found_from_either() {
        let rows = rows_of(20, 4, b"prompt>\r\nsee https://example.com/a/bc?x=1\r\ndone");
        let want =
            Found { kind: Kind::Url, text: "https://example.com/a/bc?x=1".into(), start: At { x: 4, y: 1 }, end: At { x: 11, y: 2 } };
        assert_eq!(at(&rows, 1, 6), Some(want.clone()));
        assert_eq!(at(&rows, 2, 3), Some(want));
        assert_eq!(at(&rows, 0, 2), None);
        assert_eq!(at(&rows, 3, 1), None);
    }

    #[test]
    fn a_link_cut_off_below_the_viewport_is_not_offered() {
        let rows = rows_of(20, 4, b"prompt>\r\nsee https://examplexy/a/bc?x=1\r\ndone");
        assert!(rows[1].wraps && rows[2].wrap_continuation);
        assert_eq!(at(&rows[..2], 1, 6), None);
    }

    #[test]
    fn rows_a_scroll_region_left_continuing_a_row_that_does_not_wrap_offer_no_link() {
        let rows = rows_of(6, 5, b"top\r\n\x1b[2;4r\x1b[4;1Hsrc/abcdefghijklmnopqrstuvwxyz");
        assert!(rows[1].wrap_continuation && !rows[0].wraps, "the region's top row continues a row that does not wrap");
        for y in 0..rows.len() {
            for x in 0..6 {
                assert_eq!(at(&rows, y, x), None, "at {x},{y}");
            }
        }
    }
}
