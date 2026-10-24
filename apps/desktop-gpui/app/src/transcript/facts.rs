// SPDX-License-Identifier: AGPL-3.0-only
//! What a call row reads off its call: its preview, its patch, its facts.

use crate::words::{server_tool_words, tool_row, ToolItem as Item, ToolRequest as Request};
use wsp_host_client::protocol::SessionEventPatchItem;

use super::{FilePatch, Patch};

/// compactLines in adapt/session.ts: each non-blank line with its runs of space as one.
pub(super) fn compact_lines(text: &str) -> Vec<String> {
    text.lines().map(|line| line.split_whitespace().collect::<Vec<_>>().join(" ")).filter(|line| !line.is_empty()).collect()
}

/// summarizeOutput in adapt/session.ts: the first line worth reading, cut at 84 characters.
pub(super) fn preview(text: &str) -> Option<String> {
    let mut fences = 0;
    for line in text.lines() {
        let mut words = line.split_whitespace();
        let Some(first) = words.next() else { continue };
        if first == "```" && words.clone().next().is_none() {
            fences += 1;
            continue;
        }
        // Stops at 85 characters: a thinking block's preview runs again on every delta.
        let mut said = String::new();
        let mut count = 0;
        for word in std::iter::once(first).chain(words) {
            if count > 0 {
                said.push(' ');
                count += 1;
            }
            for ch in word.chars().take(85 - count) {
                said.push(ch);
                count += 1;
            }
            if count > 84 {
                break;
            }
        }
        if count <= 84 {
            return Some(said);
        }
        let cut: String = said.chars().take(83).collect();
        return Some(format!("{}…", cut.trim_end()));
    }
    (fences > 1).then(|| format!("{fences} lines"))
}

pub(super) fn command_line(command: &str) -> String {
    compact_lines(command).into_iter().next().unwrap_or_else(|| command.trim().to_owned())
}

fn relative(path: &str, cwd: Option<&str>) -> String {
    let rest = cwd.and_then(|root| path.strip_prefix(root.trim_end_matches('/'))).and_then(|rest| rest.strip_prefix('/'));
    rest.unwrap_or(path).to_owned()
}

/// An edit's patch as one unified diff, with the heading its block draws (editHeading in timeline/callFacts.ts).
pub(super) fn unified(files: &[SessionEventPatchItem], cwd: Option<&str>) -> Option<Patch> {
    let created = |f: &SessionEventPatchItem| matches!(f.hunks.as_slice(), [h] if h.old_start == 0 && h.old_lines == 0);
    let gone = |f: &SessionEventPatchItem| matches!(f.hunks.as_slice(), [h] if h.new_start == 0 && h.new_lines == 0);
    let (verb, what, new) = match files {
        [] => return None,
        [one] => match &one.moved_to {
            Some(to) => ("Moved", format!("{} to {}", relative(&one.path, cwd), relative(to, cwd)), false),
            None if created(one) => ("Wrote", relative(&one.path, cwd), true),
            None => (if gone(one) { "Deleted" } else { "Edited" }, relative(&one.path, cwd), false),
        },
        many => ("Edited", format!("{} files", many.len()), false),
    };
    let mut text = Vec::new();
    let mut each = Vec::new();
    for file in files {
        let from = text.len();
        let path = file.path.strip_prefix('/').unwrap_or(&file.path);
        let to = file.moved_to.as_deref().map_or(path, |to| to.strip_prefix('/').unwrap_or(to));
        text.push(format!("diff --git a/{path} b/{to}"));
        if created(file) {
            text.extend(["--- /dev/null".to_owned(), format!("+++ b/{to}")]);
        } else if gone(file) {
            text.extend([format!("--- a/{path}"), "+++ /dev/null".to_owned()]);
        } else {
            text.extend([format!("--- a/{path}"), format!("+++ b/{to}")]);
        }
        for h in &file.hunks {
            text.push(format!("@@ -{},{} +{},{} @@", h.old_start, h.old_lines, h.new_start, h.new_lines));
            text.extend(h.lines.iter().cloned());
        }
        let lines = file.hunks.iter().flat_map(|h| h.lines.iter());
        let (added, removed) = lines.fold((0, 0), |(a, r), l| match l.as_bytes().first() {
            Some(b'+') => (a + 1, r),
            Some(b'-') => (a, r + 1),
            _ => (a, r),
        });
        each.push(FilePatch {
            path: relative(file.moved_to.as_deref().unwrap_or(&file.path), cwd),
            text: text[from..].join("\n"),
            added,
            removed,
            created: created(file),
            line: file.hunks.first().map_or(1, |h| (h.new_start as u64).max(1)),
        });
    }
    Some(Patch { verb, what, one: files.len() == 1, created: new, text: text.join("\n"), files: each })
}

/// ToolCallFacts in packages/protocol/src/words/tools.ts.
#[derive(Default)]
pub(super) struct Facts {
    pub(super) title: Option<String>,
    pub(super) detail: Option<String>,
    pub(super) command: Option<String>,
    pub(super) description: Option<String>,
    pub(super) changed: Vec<String>,
    pub(super) item: Option<Item>,
    pub(super) request: Option<Request>,
}

const SHOWN_FIELDS: [&str; 7] = ["file_path", "notebook_path", "pattern", "query", "url", "description", "prompt"];

pub(super) fn facts(name: &str, input: &str, cwd: Option<&str>) -> Facts {
    let row = tool_row(name);
    let lent = if row.is_none() { server_tool_words(name) } else { None };
    let item = row.as_ref().and_then(|r| r.item).or(lent.as_ref().map(|_| Item::Mcp));
    let request = row.as_ref().and_then(|r| r.request);
    let title = row.as_ref().and_then(|r| r.title.map(str::to_owned)).or(lent.clone());
    let Some(fields) = serde_json::from_str::<serde_json::Value>(input).ok().filter(serde_json::Value::is_object) else {
        return Facts { title, item, request, detail: (!input.is_empty()).then(|| input.to_owned()), ..Facts::default() };
    };
    let field = |name: &str| fields.get(name).and_then(serde_json::Value::as_str).filter(|v| !v.trim().is_empty()).map(str::to_owned);
    let server: &[&str] = if lent.is_some() { &["title", "message"] } else { &[] };
    let shows = row.as_ref().map_or(&[][..], |r| r.shows);
    let detail = shows.iter().chain(SHOWN_FIELDS.iter()).chain(server.iter()).find_map(|name| field(name));
    let shell = request == Some(Request::Command);
    let changed = match row.as_ref().and_then(|r| r.path) {
        Some("changes") => fields
            .get("changes")
            .and_then(serde_json::Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(|change| change.get("path")?.as_str().filter(|p| !p.is_empty()).map(|p| relative(p, cwd)))
            .collect(),
        Some(path) => field(path).map(|p| relative(&p, cwd)).into_iter().collect(),
        None => Vec::new(),
    };
    Facts {
        title,
        detail,
        command: if shell { field("command") } else { None },
        description: if shell { field("description") } else { None },
        changed,
        item,
        request,
    }
}
