// SPDX-License-Identifier: AGPL-3.0-only
//! The rc lines that read another file by a literal path with nothing around them, as rustup writes
//! `. "$HOME/.cargo/env"`. Port of packages/collect/src/detect/sources.ts at a3b870a6e; the spec is
//! spec/detect_agents.md.
use crate::detect::shell::RC_FILES;
use crate::detect::shell_rc::{source_command, sourced_path};
use crate::host::{expand, tilde, Host};
use crate::manifest::ManifestEntry;

struct Bare {
    line: usize,
    /// `~`-relative under home, else absolute.
    path: String,
    indent: String,
    word: String,
    /// The path as written, quotes included, a literal home prefix rewritten as `"$HOME"`.
    token: String,
    /// Whether the token was rewritten, so the line is never shipped as written.
    rewritten: bool,
    comment: String,
}

/// JavaScript's `split(/\r?\n/)`: a carriage return right before a line feed belongs to the separator.
fn split_lines(text: &str) -> Vec<&str> {
    let mut lines: Vec<&str> = text.split('\n').collect();
    let last = lines.len() - 1;
    for line in &mut lines[..last] {
        if let Some(stripped) = line.strip_suffix('\r') {
            *line = stripped;
        }
    }
    lines
}

/// `/^[ \t]*(#.*)?$/` on the text after a source token: the comment it holds (empty for none), or `None` when shell
/// follows. JavaScript's `.` stops at a line terminator, so a comment holding one is not a comment line.
fn tail_comment(rest: &str) -> Option<String> {
    let t = rest.trim_start_matches([' ', '\t']);
    if t.is_empty() {
        return Some(String::new());
    }
    if t.starts_with('#') && !t.contains(['\n', '\r', '\u{2028}', '\u{2029}']) {
        return Some(t.to_string());
    }
    None
}

/// The token with a literal home prefix written as `"$HOME"`; the token's own quoting stays around the rest.
fn portable(token: &str, home: &str) -> String {
    let quote = token.chars().next().filter(|c| *c == '"' || *c == '\'');
    let raw = match quote {
        Some(_) => &token[1..token.len() - 1],
        None => token,
    };
    let Some(tail) = raw.strip_prefix(format!("{home}/").as_str()) else {
        return token.to_string();
    };
    let rest = format!("/{tail}");
    match quote {
        Some('"') => format!("\"$HOME{rest}\""),
        Some(q) => format!("\"$HOME\"{q}{rest}{q}"),
        None => format!("\"$HOME\"{rest}"),
    }
}

fn scan(text: &str, home: &str) -> Vec<Bare> {
    let mut out = Vec::new();
    for (line, content) in split_lines(text).into_iter().enumerate() {
        let Some(cmd) = source_command(content) else {
            continue;
        };
        let Some(comment) = tail_comment(&cmd.rest) else {
            continue;
        };
        let Some(abs) = sourced_path(&cmd.token, home) else {
            continue;
        };
        let token = portable(&cmd.token, home);
        out.push(Bare { line, path: tilde(home, &abs), indent: cmd.indent, word: cmd.word, rewritten: token != cmd.token, token, comment });
    }
    out
}

/// The files an rc file reads on a bare line, `~`-relative under home and absolute outside it, once each, in the order
/// they first appear.
pub fn bare_sources(text: &str, home: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for b in scan(text, home) {
        if !out.contains(&b.path) {
            out.push(b.path);
        }
    }
    out
}

/// The text with each bare source line whose file is not on the machine wrapped as `[ -r path ] && . path`; every
/// other byte as it was.
pub fn guard_sources(text: &str, home: &str, present: &dyn Fn(&str) -> bool) -> String {
    let bare: Vec<Bare> =
        scan(text, home).into_iter().filter(|b| b.rewritten || !b.path.starts_with("~/") || !present(&b.path[2..])).collect();
    if bare.is_empty() {
        return text.to_string();
    }
    let eol = if text.contains("\r\n") { "\r\n" } else { "\n" };
    let mut lines: Vec<String> = split_lines(text).into_iter().map(str::to_string).collect();
    for b in bare {
        let comment = if b.comment.is_empty() { String::new() } else { format!(" {}", b.comment) };
        lines[b.line] = format!("{}[ -r {} ] && {} {}{}", b.indent, b.token, b.word, b.token, comment);
    }
    lines.join(eol)
}

/// Puts on each zsh or bash rc row (shell/<RC_FILES but inputrc>) the files it reads on a bare line.
pub async fn shell_sources(host: &Host, rows: &mut [ManifestEntry]) {
    for row in rows.iter_mut() {
        let Some(path) = row.paths.first().cloned() else {
            continue;
        };
        if !RC_FILES.iter().any(|n| *n != "inputrc" && row.id == format!("shell/{n}")) {
            continue;
        }
        let Some(text) = host.fs.read_text(&expand(&host.home, &path)).await else {
            continue;
        };
        let sources = bare_sources(&text, &host.home);
        if !sources.is_empty() {
            row.sources = Some(sources);
        }
    }
}
