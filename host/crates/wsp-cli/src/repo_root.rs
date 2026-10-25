// SPDX-License-Identifier: AGPL-3.0-only
//! The repository a folder sits in, and its main folder when the folder is in a linked worktree. Port of
//! `packages/host/src/repo-root.ts` at 65f6b89c7; the spec is `spec/account.md`.
use std::fs;
use std::path::Path;
use std::sync::LazyLock;

use regex::Regex;
use wsp_words::js::js_trim;

/// Node's `path.posix` for the strings here: `.` and empty parts go, `..` takes back the part before it, and a `..`
/// with nothing before it is kept only where the path is relative.
fn segments(path: &str, allow_above_root: bool) -> Vec<&str> {
    let mut out: Vec<&str> = Vec::new();
    for seg in path.split('/') {
        match seg {
            "" | "." => {}
            ".." if out.last().is_some_and(|last| *last != "..") => {
                out.pop();
            }
            ".." if allow_above_root => out.push(".."),
            ".." => {}
            other => out.push(other),
        }
    }
    out
}

/// `path.posix.normalize`: a trailing slash is kept, an empty result is `/`, `./` or `.`.
pub(crate) fn normalize(path: &str) -> String {
    if path.is_empty() {
        return ".".into();
    }
    let absolute = path.starts_with('/');
    let inner = segments(path, !absolute).join("/");
    if inner.is_empty() {
        return if absolute {
            "/".into()
        } else if path.ends_with('/') {
            "./".into()
        } else {
            ".".into()
        };
    }
    let mut out = if absolute { format!("/{inner}") } else { inner };
    if path.ends_with('/') {
        out.push('/');
    }
    out
}

/// `path.posix.join` over the non-empty parts.
pub(crate) fn join(parts: &[&str]) -> String {
    let joined: Vec<&str> = parts.iter().copied().filter(|p| !p.is_empty()).collect();
    if joined.is_empty() {
        return ".".into();
    }
    normalize(&joined.join("/"))
}

/// `path.posix.dirname`.
pub(crate) fn dirname(path: &str) -> String {
    if path.is_empty() {
        return ".".into();
    }
    let bytes = path.as_bytes();
    let has_root = bytes[0] == b'/';
    let mut end = None;
    let mut matched_slash = true;
    for i in (1..bytes.len()).rev() {
        if bytes[i] == b'/' {
            if !matched_slash {
                end = Some(i);
                break;
            }
        } else {
            matched_slash = false;
        }
    }
    match end {
        None => if has_root { "/" } else { "." }.into(),
        Some(1) if has_root => "//".into(),
        Some(at) => path[..at].to_string(),
    }
}

/// `path.posix.basename` without an extension to take off.
pub(crate) fn basename(path: &str) -> String {
    path.trim_end_matches('/').rsplit('/').next().unwrap_or("").to_string()
}

/// `path.posix.resolve(cwd, path)` for an absolute `cwd`: no trailing slash, `/` for the root.
pub(crate) fn resolve(cwd: &str, path: &str) -> String {
    let whole = if path.starts_with('/') {
        path.to_string()
    } else if path.is_empty() {
        cwd.to_string()
    } else {
        format!("{cwd}/{path}")
    };
    let absolute = whole.starts_with('/');
    let inner = segments(&whole, !absolute).join("/");
    if absolute {
        format!("/{inner}")
    } else if inner.is_empty() {
        ".".into()
    } else {
        inner
    }
}

/// repo-root.ts:9-14: the nearest folder up from `folder`, itself included, holding a `.git` entry of any kind.
pub fn git_root_of(folder: &str) -> Option<String> {
    let mut at = folder.to_string();
    loop {
        if Path::new(&join(&[at.as_str(), ".git"])).exists() {
            return Some(at);
        }
        let up = dirname(&at);
        if up == at {
            return None;
        }
        at = up;
    }
}

/// A `.git` file with no `gitdir:` line, or pointing anywhere but `<main>/.git/worktrees/<name>`,
/// is `None`; an unreadable `.git` file is an error in the TS (it throws), `None` here.
pub fn main_worktree_of(folder: &str) -> Option<String> {
    static GITDIR: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?m)^gitdir:\s*(.+)$").unwrap());
    let root = git_root_of(folder)?;
    let dot_git = join(&[root.as_str(), ".git"]);
    if fs::metadata(&dot_git).ok()?.is_dir() {
        return Some(root);
    }
    let text = fs::read_to_string(&dot_git).ok()?;
    let pointed = js_trim(GITDIR.captures(&text)?.get(1)?.as_str());
    let git_dir = if pointed.starts_with('/') { pointed.to_string() } else { resolve(&root, pointed) };
    let worktrees = dirname(&git_dir);
    if worktrees.ends_with("/.git/worktrees") {
        Some(dirname(&dirname(&worktrees)))
    } else {
        None
    }
}
