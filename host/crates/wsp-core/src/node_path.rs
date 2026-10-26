// SPDX-License-Identifier: AGPL-3.0-only
//! Node's `path.join` for POSIX paths: the parts joined with `/`, then `.`, `..` and doubled slashes folded away.

/// `path.posix.join(...parts)`: empty parts skipped, `.` where nothing is left.
pub fn join(parts: &[&str]) -> String {
    let parts: Vec<&str> = parts.iter().copied().filter(|p| !p.is_empty()).collect();
    if parts.is_empty() {
        return ".".to_string();
    }
    normalize(&parts.join("/"))
}

fn normalize(path: &str) -> String {
    let absolute = path.starts_with('/');
    let trailing = path.ends_with('/');
    let mut kept: Vec<&str> = Vec::new();
    for part in path.split('/') {
        match part {
            "" | "." => {}
            ".." if kept.last().is_some_and(|last| *last != "..") => {
                kept.pop();
            }
            ".." if !absolute => kept.push(".."),
            ".." => {}
            part => kept.push(part),
        }
    }
    let mut folded = kept.join("/");
    if folded.is_empty() && !absolute {
        folded.push('.');
    }
    if trailing && !folded.is_empty() {
        folded.push('/');
    }
    if absolute {
        format!("/{folded}")
    } else {
        folded
    }
}
