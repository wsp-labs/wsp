// SPDX-License-Identifier: AGPL-3.0-only
//! What the tools table's rows and a tool call's input are read with: the private half of tools.rs.

use super::*;

pub(super) const SHOWN_FIELDS: &[&str] = &["file_path", "notebook_path", "pattern", "query", "url", "description", "prompt"];

pub(super) const SERVER_SHOWN_FIELDS: &[&str] = &["title", "message"];

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Moment {
    Asked,
    Done,
}

#[derive(Clone, Copy)]
pub(super) enum Line {
    Shell,
    Path { doing: &'static str, did: &'static str, field: &'static str },
    About { doing: &'static str, did: Option<&'static str>, field: &'static str },
    Changes,
    Question,
}

#[derive(Clone, Copy)]
pub(super) struct ToolRow {
    pub(super) line: Line,
    pub(super) title: Option<&'static str>,
    pub(super) item_type: Option<ToolItemType>,
    pub(super) request_kind: Option<ToolRequestKind>,
    pub(super) code_search: bool,
}

pub(super) fn row(line: Line) -> ToolRow {
    ToolRow { line, title: None, item_type: None, request_kind: None, code_search: false }
}

pub(super) fn tool_row(name: &str) -> Option<ToolRow> {
    let file_change = Some(ToolItemType::FileChange);
    let file_kind = Some(ToolRequestKind::FileChange);
    Some(match name {
        "Bash" | "command_execution" => {
            ToolRow { item_type: Some(ToolItemType::CommandExecution), request_kind: Some(ToolRequestKind::Command), ..row(Line::Shell) }
        }
        "Read" => ToolRow {
            request_kind: Some(ToolRequestKind::FileRead),
            ..row(Line::Path { doing: "reading", did: "read", field: "file_path" })
        },
        "Write" => ToolRow {
            item_type: file_change,
            request_kind: file_kind,
            ..row(Line::Path { doing: "writing", did: "wrote", field: "file_path" })
        },
        "Edit" | "MultiEdit" => ToolRow {
            item_type: file_change,
            request_kind: file_kind,
            ..row(Line::Path { doing: "editing", did: "edited", field: "file_path" })
        },
        "NotebookEdit" => ToolRow {
            item_type: file_change,
            request_kind: file_kind,
            ..row(Line::Path { doing: "editing", did: "edited", field: "notebook_path" })
        },
        "file_change" => ToolRow { item_type: file_change, request_kind: file_kind, ..row(Line::Changes) },
        "Grep" | "Glob" => ToolRow {
            code_search: true,
            ..row(Line::About { doing: "searching code for", did: Some("searched code for"), field: "pattern" })
        },
        "WebSearch" | "web_search" => ToolRow {
            item_type: Some(ToolItemType::WebSearch),
            ..row(Line::About { doing: "searching the web for", did: Some("searched the web for"), field: "query" })
        },
        "WebFetch" => ToolRow {
            item_type: Some(ToolItemType::WebSearch),
            ..row(Line::About { doing: "fetching", did: Some("fetched"), field: "url" })
        },
        "Task" | "Agent" => ToolRow {
            item_type: Some(ToolItemType::CollabAgentToolCall),
            ..row(Line::About { doing: "agent:", did: None, field: "description" })
        },
        "spawn_agent" => ToolRow {
            item_type: Some(ToolItemType::CollabAgentToolCall),
            ..row(Line::About { doing: "agent:", did: None, field: "prompt" })
        },
        QUESTION_TOOL => ToolRow { title: Some("Asked you"), ..row(Line::Question) },
        _ => return None,
    })
}

impl ToolRow {
    pub(super) fn shows(&self) -> Vec<&'static str> {
        match self.line {
            Line::Shell => vec!["description"],
            Line::Path { field, .. } | Line::About { field, .. } => vec![field],
            Line::Changes | Line::Question => vec![],
        }
    }

    pub(super) fn text(&self, fields: &ToolInput, moment: Moment) -> Option<String> {
        let done = moment == Moment::Done;
        match self.line {
            Line::Shell => tool_field(fields, "command").map(|command| format!("$ {}", title_line(command))),
            Line::Path { doing, did, field } => tool_field(fields, field).map(|path| format!("{} {path}", if done { did } else { doing })),
            Line::About { doing, did, field } => tool_field(fields, field).map(|what| {
                let word = if done { did.unwrap_or(doing) } else { doing };
                format!("{word} {}", title_line(what))
            }),
            Line::Changes => {
                let verb = if done { "edited" } else { "editing" };
                match changed_paths(fields).as_slice() {
                    [] => None,
                    [path] => Some(format!("{verb} {path}")),
                    paths => Some(format!("{verb} {}", plural(paths.len() as f64, "file"))),
                }
            }
            Line::Question => questions_in(fields).and_then(|questions| questions.into_iter().next()).map(|q| q.question),
        }
    }

    pub(super) fn changed(&self, fields: &ToolInput) -> Vec<String> {
        match self.line {
            Line::Changes => changed_paths(fields),
            Line::Path { field, .. } if self.request_kind == Some(ToolRequestKind::FileChange) => {
                tool_field(fields, field).map(|path| vec![path.to_string()]).unwrap_or_default()
            }
            _ => vec![],
        }
    }
}

pub(super) fn tool_input(input: &str) -> Option<ToolInput> {
    match crate::v8_json::parse_json_lax(input) {
        Some(Value::Object(fields)) => Some(js_ordered(fields)),
        _ => None,
    }
}

/// The fields as a JavaScript object lists them: the keys that read as array indexes first, in ascending number.
pub(super) fn js_ordered(fields: ToolInput) -> ToolInput {
    crate::js::js_key_order(fields.iter()).into_iter().map(|(key, value)| (key.clone(), value.clone())).collect()
}

pub(super) fn tool_field<'a>(fields: &'a ToolInput, name: &str) -> Option<&'a str> {
    fields.get(name)?.as_str().filter(|value| !js_trim(value).is_empty())
}

pub(super) fn first_field(fields: &ToolInput, names: &[&str]) -> Option<String> {
    names.iter().find_map(|name| tool_field(fields, name)).map(str::to_string)
}

pub(super) fn changed_paths(fields: &ToolInput) -> Vec<String> {
    match fields.get("changes") {
        Some(Value::Array(changes)) => changes
            .iter()
            .filter_map(|change| change.as_object()?.get("path")?.as_str().filter(|path| !path.is_empty()).map(str::to_string))
            .collect(),
        _ => vec![],
    }
}

pub(super) fn questions_in(fields: &ToolInput) -> Option<Vec<AskedQuestion>> {
    let Some(Value::Array(raw)) = fields.get("questions") else {
        return None;
    };
    let mut questions = Vec::new();
    for (index, entry) in raw.iter().enumerate() {
        let Some(question) = entry.as_object() else {
            continue;
        };
        let Some(text) = tool_field(question, "question") else {
            continue;
        };
        questions.push(AskedQuestion {
            key: text.to_string(),
            index,
            header: tool_field(question, "header").unwrap_or_default().to_string(),
            question: text.to_string(),
            options: asked_options(question.get("options"), index),
            multi_select: question.get("multiSelect") == Some(&Value::Bool(true)),
        });
    }
    (!questions.is_empty()).then_some(questions)
}

pub(super) fn asked_options(raw: Option<&Value>, question: usize) -> Vec<AskedOption> {
    let Some(Value::Array(raw)) = raw else {
        return vec![];
    };
    let mut options: Vec<AskedOption> = Vec::new();
    for entry in raw {
        let Some(fields) = entry.as_object() else {
            continue;
        };
        let Some(label) = tool_field(fields, "label") else {
            continue;
        };
        options.push(AskedOption {
            id: format!("q{question}:o{}", options.len()),
            label: label.to_string(),
            description: tool_field(fields, "description").unwrap_or_default().to_string(),
        });
    }
    options
}

/// `encodeURIComponent`: every byte outside the unreserved set as an upper-case percent escape.
pub(super) fn encode_uri_component(words: &str) -> String {
    let mut out = String::new();
    for byte in words.bytes() {
        if byte.is_ascii_alphanumeric() || b"-_.!~*'()".contains(&byte) {
            out.push(byte as char);
        } else {
            out.push_str(&format!("%{byte:02X}"));
        }
    }
    out
}

/// `decodeURIComponent`: None where an escape is malformed or the bytes are not UTF-8.
pub(super) fn decode_uri_component(text: &str) -> Option<String> {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut at = 0;
    while at < bytes.len() {
        if bytes[at] == b'%' {
            let hex = text.get(at + 1..at + 3).filter(|hex| hex.bytes().all(|b| b.is_ascii_hexdigit()))?;
            out.push(u8::from_str_radix(hex, 16).ok()?);
            at += 3;
        } else {
            out.push(bytes[at]);
            at += 1;
        }
    }
    String::from_utf8(out).ok()
}

/// `/^q(\d+):other:(.*)$/` read back as its question number, which `Number` reads as a float however long, and decoded words.
pub(super) fn typed_answer(id: &str) -> Option<(f64, String)> {
    let rest = id.strip_prefix('q')?;
    let digits = rest.bytes().take_while(u8::is_ascii_digit).count();
    if digits == 0 {
        return None;
    }
    let question = rest[..digits].parse().ok()?;
    let words = rest[digits..].strip_prefix(":other:")?;
    if words.contains(['\n', '\r', '\u{2028}', '\u{2029}']) {
        return None;
    }
    Some((question, decode_uri_component(words)?))
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum AskKind {
    Write,
    Edit,
    Command,
    CodexCommand,
    FileChange,
    Fetch,
    Skill,
    Question,
    Server,
    Plain,
}

pub(super) fn ask_kind(tool_name: &str) -> AskKind {
    match tool_name {
        "Write" => AskKind::Write,
        "Edit" | "MultiEdit" => AskKind::Edit,
        "Bash" => AskKind::Command,
        "WebFetch" => AskKind::Fetch,
        "Skill" => AskKind::Skill,
        QUESTION_TOOL => AskKind::Question,
        "command_execution" => AskKind::CodexCommand,
        "file_change" => AskKind::FileChange,
        _ if server_tool(tool_name).is_some() => AskKind::Server,
        _ => AskKind::Plain,
    }
}

pub(super) fn named_fields(kind: AskKind) -> &'static [&'static str] {
    match kind {
        AskKind::Write => &["file_path", "content"],
        AskKind::Edit => &["file_path", "old_string", "new_string", "replace_all", "edits"],
        AskKind::Command => &["command", "description"],
        AskKind::CodexCommand => &["command", "cwd"],
        AskKind::FileChange => &["changes"],
        AskKind::Fetch => &["url", "prompt"],
        AskKind::Skill => &["skill"],
        AskKind::Question => &["questions"],
        AskKind::Server | AskKind::Plain => &[],
    }
}

/// `health.ts in src`, or the name alone at the root.
pub(super) fn file_in_folder(path: &str) -> String {
    let folder = parent_folder_name(path);
    let name = folder_name(path);
    if folder.is_empty() {
        name
    } else {
        format!("{name} in {folder}")
    }
}

pub(super) struct AskLead {
    pub(super) says: String,
    pub(super) code: Option<String>,
}

pub(super) fn lead_for(kind: AskKind, tool_name: &str, fields: &ToolInput) -> Option<AskLead> {
    let says = |says: String| Some(AskLead { says, code: None });
    let command = |code: &str| Some(AskLead { says: "Run a command:".to_string(), code: Some(code.to_string()) });
    match kind {
        AskKind::Write => {
            let path = tool_field(fields, "file_path")?;
            let size = match fields.get("content").and_then(Value::as_str) {
                Some(content) => format!(" ({})", fmt_bytes(content.len() as f64)),
                None => String::new(),
            };
            says(format!("Write {}{size}", file_in_folder(path)))
        }
        AskKind::Edit => {
            let path = tool_field(fields, "file_path")?;
            let count = match fields.get("edits") {
                Some(Value::Array(edits)) if edits.len() > 1 => format!(" ({} places)", edits.len()),
                _ => String::new(),
            };
            says(format!("Edit {}{count}", file_in_folder(path)))
        }
        AskKind::Command | AskKind::CodexCommand => command(tool_field(fields, "command")?),
        AskKind::FileChange => {
            let Some(Value::Array(changes)) = fields.get("changes") else {
                return None;
            };
            let paths: Vec<&str> = changes.iter().filter_map(|change| tool_field(change.as_object()?, "path")).collect();
            match paths.as_slice() {
                [] => None,
                [path] => says(format!("Change {}", file_in_folder(path))),
                paths => says(format!(
                    "Change {} files: {}",
                    paths.len(),
                    paths.iter().map(|path| folder_name(path)).collect::<Vec<_>>().join(", ")
                )),
            }
        }
        AskKind::Fetch => Some(AskLead { says: "Fetch a page:".to_string(), code: Some(tool_field(fields, "url")?.to_string()) }),
        AskKind::Skill => says(format!("Run the skill {}", tool_field(fields, "skill")?)),
        AskKind::Question => questions_in(fields)?.into_iter().next().and_then(|question| says(question.question)),
        AskKind::Server => server_tool_words(tool_name).and_then(says),
        AskKind::Plain => None,
    }
}

pub(super) fn ask_lead(tool_name: &str, input: &str, detail: Option<&str>) -> AskLead {
    let lead = tool_input(input).and_then(|fields| lead_for(ask_kind(tool_name), tool_name, &fields));
    lead.unwrap_or_else(|| AskLead {
        says: match detail {
            None | Some("") => format!("Permission for {tool_name}"),
            Some(detail) => format!("Permission for {tool_name}: {detail}"),
        },
        code: None,
    })
}

pub(super) fn lead_line(lead: &AskLead) -> String {
    match &lead.code {
        None => lead.says.clone(),
        Some(code) => format!("{} {code}", lead.says),
    }
}

/// `JSON.stringify` of a parsed value: keys in the order written, numbers as `String(n)` writes them.
pub(super) fn js_json(value: &Value) -> String {
    match value {
        Value::Null => "null".to_string(),
        Value::Bool(flag) => flag.to_string(),
        Value::Number(number) => number_string(number.as_f64().unwrap_or(f64::NAN)),
        Value::String(text) => serde_json::to_string(text).unwrap_or_default(),
        Value::Array(items) => format!("[{}]", items.iter().map(js_json).collect::<Vec<_>>().join(",")),
        Value::Object(fields) => format!(
            "{{{}}}",
            fields
                .iter()
                .map(|(key, value)| format!("{}:{}", serde_json::to_string(key).unwrap_or_default(), js_json(value)))
                .collect::<Vec<_>>()
                .join(",")
        ),
    }
}

/// One value as the row reads it: its own whitespace collapsed, so a field holding a paragraph is one line.
pub(super) fn rest_value(value: &Value) -> String {
    let raw = match value {
        Value::String(text) => text.clone(),
        Value::Array(items) if items.iter().all(|item| matches!(item, Value::String(_) | Value::Number(_))) => items
            .iter()
            .map(|item| match item {
                Value::String(text) => text.clone(),
                Value::Number(number) => number_string(number.as_f64().unwrap_or(f64::NAN)),
                _ => String::new(),
            })
            .collect::<Vec<_>>()
            .join(", "),
        other => js_json(other),
    };
    js_trim(&js_collapse_whitespace(&raw)).to_string()
}

/// The strings an edit swaps, off the first of several edits, else the call's own.
pub(super) fn edit_strings(fields: &ToolInput) -> Option<PromptEdit> {
    let first = match fields.get("edits") {
        Some(Value::Array(edits)) => match edits.first() {
            Some(Value::Object(first)) => first,
            Some(Value::Array(_)) => return None,
            _ => fields,
        },
        _ => fields,
    };
    let old = first.get("old_string")?.as_str()?;
    let next = first.get("new_string")?.as_str()?;
    Some(PromptEdit { old: old.to_string(), next: next.to_string() })
}
