// SPDX-License-Identifier: AGPL-3.0-only
//! Where the agent's tool shell is, read from its tool calls. TS: packages/adapter-claude/src/shell-cwd.ts at
//! a3b870a6e. Paths are posix, normalized as node's `path.posix.normalize`, a trailing slash dropped.

use std::sync::LazyLock;

use regex::Regex;
use serde_json::Value;

use crate::claude::fields::{rec, string};
use wsp_words::js::JS_WS;

static CD_HEAD: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(&format!(r#"^[{ws}]*cd[{ws}]+(?:"([^"$`]*)"|'([^']*)'|([^{ws};&|$`*?\[{{~]+))[{ws}]*(?:$|&&|;|\|\||\n)"#, ws = JS_WS))
        .expect("the cd pattern is valid")
});

/// Node's `path.posix.normalize` for an absolute path, trailing slash dropped; `None` for a relative one.
fn absolute(path: Option<&str>) -> Option<String> {
    let path = path?;
    if !path.starts_with('/') {
        return None;
    }
    let mut parts: Vec<&str> = Vec::new();
    for segment in path.split('/') {
        match segment {
            "" | "." => {}
            ".." => {
                parts.pop();
            }
            name => parts.push(name),
        }
    }
    Some(format!("/{}", parts.join("/")))
}

fn dirname(path: &str) -> &str {
    match path.rfind('/') {
        Some(0) => "/",
        Some(i) => &path[..i],
        None => ".",
    }
}

fn within(path: &str, folder: &str) -> bool {
    let prefix = if folder == "/" { "/".to_string() } else { format!("{folder}/") };
    path == folder || path.starts_with(&prefix)
}

/// The shell's folder after this tool call when the call moves it, else undefined. shell-cwd.ts:27.
pub fn shell_cwd_after(tool_name: Option<&str>, input: Option<&Value>, current: &str, harness_cwd: &str) -> Option<String> {
    let tool_name = tool_name?;
    let fields = rec(input)?;
    if tool_name == "Bash" {
        let command = string(fields.get("command")).unwrap_or("");
        let caps = CD_HEAD.captures(command)?;
        let raw = caps.get(1).or(caps.get(2)).or(caps.get(3))?.as_str();
        let target = absolute(Some(raw))?;
        return (target != current).then_some(target);
    }
    if tool_name != "Write" && tool_name != "Edit" {
        return None;
    }
    let file = absolute(string(fields.get("file_path")))?;
    let folder = dirname(&file);
    if folder == harness_cwd || !within(folder, harness_cwd) {
        return None;
    }
    if current != harness_cwd && within(folder, current) {
        return None;
    }
    Some(folder.to_string())
}
