// SPDX-License-Identifier: AGPL-3.0-only
//! Node's `path.posix` for the two calls the host makes: `join` and `resolve`. wsp-cli keeps its own at crate level.

fn segments(path: &str, keep_above_root: bool) -> Vec<&str> {
    let mut out: Vec<&str> = Vec::new();
    for seg in path.split('/') {
        match seg {
            "" | "." => {}
            ".." if out.last().is_some_and(|last| *last != "..") => {
                out.pop();
            }
            ".." if keep_above_root => out.push(".."),
            ".." => {}
            other => out.push(other),
        }
    }
    out
}

/// `path.posix.join` over the non-empty parts.
pub fn join(parts: &[&str]) -> String {
    let joined: Vec<&str> = parts.iter().copied().filter(|p| !p.is_empty()).collect();
    if joined.is_empty() {
        return ".".into();
    }
    let whole = joined.join("/");
    let absolute = whole.starts_with('/');
    let inner = segments(&whole, !absolute).join("/");
    let mut out = match (absolute, inner.is_empty()) {
        (true, true) => "/".to_string(),
        (true, false) => format!("/{inner}"),
        (false, true) => ".".to_string(),
        (false, false) => inner,
    };
    if whole.ends_with('/') && !out.ends_with('/') {
        out.push('/');
    }
    out
}

/// `path.posix.resolve(cwd, path)` for an absolute `cwd`: no trailing slash, `/` for the root.
pub fn resolve(cwd: &str, path: &str) -> String {
    let whole = if path.starts_with('/') {
        path.to_string()
    } else if path.is_empty() {
        cwd.to_string()
    } else {
        format!("{cwd}/{path}")
    };
    format!("/{}", segments(&whole, false).join("/"))
}

/// The folder a path sits in, as `path.posix.dirname`.
pub fn dirname(path: &str) -> String {
    match path.trim_end_matches('/').rfind('/') {
        Some(0) => "/".to_string(),
        Some(i) => path[..i].to_string(),
        None => ".".to_string(),
    }
}

/// The last part of a path, as `path.posix.basename`.
pub fn basename(path: &str) -> String {
    path.trim_end_matches('/').rsplit('/').next().unwrap_or("").to_string()
}
