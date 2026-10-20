// SPDX-License-Identifier: AGPL-3.0-only
//! What a prompt row says: its lead, its inputs, its outcome.

use crate::words::{server_tool_words, tool_field, tool_input, ToolItem as Item, ToolRequest as Request};

use super::{
    facts::facts,
    model::{change_paths, file_in_folder, questions_in},
    Asked, Glyph, Prompt, PromptOption, Row, RowKind,
};

/// permissionPromptWords in packages/protocol/src/words/tools.ts: the lead, the unsaid inputs, a write's body, the questions.
pub(super) struct Words {
    pub(super) says: String,
    pub(super) code: Option<String>,
    pub(super) rest: Vec<(String, String)>,
    pub(super) body: Option<String>,
    questions: Option<Vec<Asked>>,
}

/// A prompt's lead: its words, and the call's own text where it has some.
type Lead = (String, Option<String>);

pub(super) fn prompt_words(tool: &str, input: &str, detail: Option<&str>) -> Words {
    let fields = tool_input(input);
    let field = |name: &str| fields.as_ref().and_then(|f| tool_field(f, name)).map(str::to_owned);
    let server = server_tool_words(tool);
    let (lead, named, body): (Option<Lead>, &[&str], Option<&str>) = match tool {
        "Write" => {
            let size =
                fields.as_ref().and_then(|f| f.get("content")?.as_str()).map(|c| format!(" ({})", crate::ui::fmt_bytes(c.len() as u64)));
            (
                field("file_path").map(|p| (format!("Write {}{}", file_in_folder(&p), size.unwrap_or_default()), None)),
                &["file_path", "content"],
                Some("content"),
            )
        }
        "Edit" | "MultiEdit" => {
            let edits = fields.as_ref().and_then(|f| f.get("edits")?.as_array().map(Vec::len)).filter(|n| *n > 1);
            let count = edits.map_or(String::new(), |n| format!(" ({n} places)"));
            let lead = field("file_path").map(|p| (format!("Edit {}{count}", file_in_folder(&p)), None));
            (lead, &["file_path", "old_string", "new_string", "replace_all", "edits"], None)
        }
        "Bash" => (field("command").map(|c| ("Run a command:".to_owned(), Some(c))), &["command", "description"], None),
        "command_execution" => (field("command").map(|c| ("Run a command:".to_owned(), Some(c))), &["command", "cwd"], None),
        "WebFetch" => (field("url").map(|u| ("Fetch a page:".to_owned(), Some(u))), &["url", "prompt"], None),
        "Skill" => (field("skill").map(|k| (format!("Run the skill {k}"), None)), &["skill"], None),
        "AskUserQuestion" => {
            let first = fields.as_ref().and_then(questions_in).and_then(|q| q.into_iter().next());
            (first.map(|q| (q.question, None)), &["questions"], None)
        }
        "file_change" => {
            let paths: Vec<String> = fields.as_ref().map(change_paths).unwrap_or_default();
            let says = match paths.as_slice() {
                [] => None,
                [one] => Some(format!("Change {}", file_in_folder(one))),
                many => {
                    let names: Vec<&str> = many.iter().map(|p| p.trim_end_matches('/').rsplit('/').next().unwrap_or(p)).collect();
                    Some(format!("Change {} files: {}", many.len(), names.join(", ")))
                }
            };
            (says.map(|s| (s, None)), &["changes"], None)
        }
        _ => (server.clone().filter(|_| fields.is_some()).map(|s| (s, None)), &[], None),
    };
    let (says, code) = lead.unwrap_or_else(|| match detail.filter(|d| !d.is_empty()) {
        Some(detail) => (format!("Permission for {tool}: {detail}"), None),
        None => (format!("Permission for {tool}"), None),
    });
    let questions = (tool == "AskUserQuestion").then(|| fields.as_ref().and_then(questions_in)).flatten();
    let Some(fields) = fields.filter(|_| questions.is_none()) else {
        let rest = if questions.is_some() || input.is_empty() { Vec::new() } else { vec![(String::new(), input.to_owned())] };
        return Words { says, code, rest, body: None, questions };
    };
    let rest =
        fields.iter().filter(|(key, _)| !named.contains(&key.as_str())).map(|(key, value)| (key.clone(), rest_value(value))).collect();
    let body = body.and_then(|name| tool_field(&fields, name)).map(str::to_owned);
    Words { says, code, rest, body, questions: None }
}

/// restValue: one input value as the row reads it, its whitespace collapsed.
fn rest_value(value: &serde_json::Value) -> String {
    let text = match value {
        serde_json::Value::String(s) => s.clone(),
        serde_json::Value::Array(items) if items.iter().all(|v| v.is_string() || v.is_number()) => {
            items.iter().map(|v| v.as_str().map_or_else(|| v.to_string(), str::to_owned)).collect::<Vec<_>>().join(", ")
        }
        other => other.to_string(),
    };
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// The glyph a prompt's record wears (recordGlyph in PermissionPromptRow.tsx).
pub(super) fn prompt_glyph(tool: &str, input: &str) -> Glyph {
    let found = facts(tool, input, None);
    if found.request == Some(Request::Command) || found.item == Some(Item::Command) {
        Glyph::Terminal
    } else if found.request == Some(Request::FileChange) || found.item == Some(Item::FileChange) {
        Glyph::SquarePen
    } else if found.item == Some(Item::WebSearch) {
        Glyph::Globe
    } else {
        Glyph::Wrench
    }
}

/// permissionOutcomeLine and pickedOptions in packages/protocol/src/words/tools.ts.
pub(super) fn outcome_line(p: &Prompt) -> Option<String> {
    let outcome = p.outcome.as_deref()?;
    let asks = p.options.iter().all(|(_, _, effect, _)| effect == "answer");
    let picked: Option<Vec<(String, String)>> = p
        .picked
        .as_deref()
        .unwrap_or("")
        .split('|')
        .map(|id| {
            p.options.iter().find(|(o, _, _, _)| o == id).map(|(_, label, effect, _)| (label.clone(), effect.clone())).or_else(|| {
                let typed = id.strip_prefix('q')?.split_once(":other:")?;
                (asks && typed.0.parse::<u32>().is_ok()).then(|| unescape(typed.1)).flatten().map(|words| (words, "answer".to_owned()))
            })
        })
        .collect();
    let named =
        picked.filter(|p| !p.is_empty()).map(|p| (p.iter().map(|(l, _)| l.as_str()).collect::<Vec<_>>().join(", "), p[0].1.clone()));
    let mode = named.as_ref().filter(|(_, effect)| effect == "mode").map_or(String::new(), |(label, _)| format!(": {label}"));
    Some(match outcome {
        "allowed" => match &named {
            Some((label, effect)) if effect == "answer" => format!("You answered: {label}"),
            _ => format!("Allowed{mode}"),
        },
        "denied" => format!("Denied{mode}"),
        "unanswered" => "Nobody answered; denied".to_owned(),
        "cancelled" => "Cancelled with the turn".to_owned(),
        _ => String::new(),
    })
}

/// decodeURIComponent: None where an escape is not whole UTF-8.
fn unescape(text: &str) -> Option<String> {
    let mut out = Vec::new();
    let raw = text.as_bytes();
    let mut i = 0;
    while i < raw.len() {
        if raw[i] == b'%' {
            out.push(u8::from_str_radix(text.get(i + 1..i + 3)?, 16).ok()?);
            i += 3;
        } else {
            out.push(raw[i]);
            i += 1;
        }
    }
    String::from_utf8(out).ok()
}

/// `askingLine`: a prompt's lead as one line, the call's own text after its words.
pub(crate) fn asking_line(tool: &str, input: &str, detail: Option<&str>) -> String {
    let words = prompt_words(tool, input, detail);
    match words.code {
        Some(code) => format!("{} {code}", words.says),
        None => words.says,
    }
}

/// One prompt as its row: a question the call puts to the person, or the consent it asks for.
pub(super) fn prompt_row(key: &str, turn: Option<String>, p: &Prompt, asker: Option<String>) -> Row {
    let words = prompt_words(&p.tool, &p.input, p.detail.as_deref());
    let outcome = outcome_line(p);
    let (ask, session) = (p.ask.clone(), p.session.clone());
    if let Some(questions) = words.questions {
        let kind = RowKind::Question { ask, session, asker, questions, outcome };
        return Row { key: key.to_owned(), kind, text: words.says, turn };
    }
    let text = match &words.code {
        Some(code) => format!("{} {code}", words.says),
        None => words.says.clone(),
    };
    let options = p
        .options
        .iter()
        .map(|(id, label, effect, mode)| PromptOption {
            id: id.clone(),
            label: label.clone(),
            description: None,
            effect: effect.clone(),
            mode: mode.clone(),
        })
        .collect();
    let kind = RowKind::Permission {
        ask,
        session,
        asker,
        glyph: prompt_glyph(&p.tool, &p.input),
        lead: words.says,
        code: words.code,
        fields: words.rest,
        file: words.body,
        options,
        open: outcome.is_none(),
        outcome,
    };
    Row { key: key.to_owned(), kind, text, turn }
}
