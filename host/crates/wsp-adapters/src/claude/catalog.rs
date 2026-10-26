// SPDX-License-Identifier: AGPL-3.0-only
//! What the installed binary reports about itself. TS: packages/adapter-claude/src/catalog.ts at a3b870a6e.

use indexmap::IndexMap;
use regex::Regex;
use serde_json::{Map, Value};

use crate::port::{program_word, AgentLaunch, HarnessCatalogModelProbe, HarnessCatalogProbe, ScreenCommand};
use wsp_wire::types::shell_quote::ENV_FROM_INPUT;
use wsp_wire::views::harness::ScreenControl;
use wsp_words::js::{js_trim, utf16};
use wsp_words::v8_json::parse_json_lax;

const SEP: &str = "__WSP_CATALOG_SEP__";
const INIT_REQUEST: &str = r#"{"type":"control_request","request_id":"init","request":{"subtype":"initialize","hooks":{}}}"#;
const ONE_M: &str = "[1m]";

/// catalog.ts:20, in that order.
pub fn claude_screen_commands() -> Vec<ScreenCommand> {
    [
        ("login", ScreenControl::SignIn),
        ("logout", ScreenControl::SignIn),
        ("model", ScreenControl::Model),
        ("permissions", ScreenControl::Access),
        ("config", ScreenControl::Settings),
        ("help", ScreenControl::Docs),
    ]
    .into_iter()
    .map(|(name, control)| ScreenCommand { name: name.to_string(), control })
    .collect()
}

/// catalog.ts:29.
pub const CONTEXT_WINDOWS: [&str; 2] = ["200k", "1m"];

/// catalog.ts:45.
pub fn catalog_probe_command(launch: Option<&AgentLaunch>) -> String {
    let claude = program_word("claude", launch);
    let init = INIT_REQUEST.replace('"', "\\\"");
    let handshake =
        format!("printf '%s\\n' \"{init}\" | {claude} -p --bare --output-format stream-json --input-format stream-json --verbose");
    format!("{}; echo {SEP}; {claude} --help; echo {SEP}; {handshake}", version_probe_command(launch))
}

/// catalog.ts:53.
pub fn version_probe_command(launch: Option<&AgentLaunch>) -> String {
    format!(
        "cd ~ && unset ${{!CLAUDE_CODE_@}} CLAUDECODE FORCE_CODE_TERMINAL; {ENV_FROM_INPUT}; {} --version",
        program_word("claude", launch)
    )
}

/// catalog.ts:59: the first `\d+\.\d+\.\d+`.
pub fn parse_version(stdout: &str) -> Option<String> {
    Regex::new(r"[0-9]+\.[0-9]+\.[0-9]+").ok()?.find(stdout).map(|found| found.as_str().to_string())
}

/// The string at a key, where it is a non-empty string.
fn str_of(value: Option<&Value>) -> Option<&str> {
    value.and_then(Value::as_str).filter(|text| !text.is_empty())
}

fn context_windows() -> Vec<String> {
    CONTEXT_WINDOWS.iter().map(|window| window.to_string()).collect()
}

/// The first object of the section that answers the initialize request, its inner response; an array counts and ends
/// the walk, a string or null is passed over.
fn init_response(section: &str) -> Option<Value> {
    for raw in section.split('\n') {
        let line = js_trim(raw);
        if !line.starts_with('{') {
            continue;
        }
        let Some(Value::Object(outer)) = parse_json_lax(line) else { continue };
        if outer.get("type").and_then(Value::as_str) != Some("control_response") {
            continue;
        }
        let Some(Value::Object(response)) = outer.get("response") else { continue };
        if let Some(inner @ (Value::Object(_) | Value::Array(_))) = response.get("response") {
            return Some(inner.clone());
        }
    }
    None
}

/// catalog.ts:85: the models the handshake names, merged by slug; `None` where none is named.
fn models_of(init: &Map<String, Value>) -> Option<Vec<HarnessCatalogModelProbe>> {
    let Some(Value::Array(entries)) = init.get("models") else { return None };
    let mut by_slug: IndexMap<String, HarnessCatalogModelProbe> = IndexMap::new();
    let mut default_slug: Option<String> = None;
    for entry in entries {
        let Some(entry) = entry.as_object() else { continue };
        let Some(value) = str_of(entry.get("value")) else { continue };
        let resolved = str_of(entry.get("resolvedModel")).unwrap_or(value);
        let slug = resolved.strip_suffix(ONE_M).unwrap_or(resolved).to_string();
        let one_m = value.ends_with(ONE_M) || resolved.ends_with(ONE_M);
        if value == "default" {
            default_slug = Some(slug);
            continue;
        }
        let efforts: Vec<String> = match entry.get("supportedEffortLevels") {
            Some(Value::Array(items)) => items.iter().filter_map(|item| item.as_str().map(String::from)).collect(),
            _ => vec![],
        };
        if let Some(known) = by_slug.get_mut(&slug) {
            let list = known.efforts.get_or_insert_with(Vec::new);
            for effort in efforts {
                if !list.contains(&effort) {
                    list.push(effort);
                }
            }
            if one_m && known.context_windows.is_empty() {
                known.context_windows = context_windows();
            }
            continue;
        }
        let label = without_one_m_context(str_of(entry.get("displayName")).unwrap_or(&slug)).to_string();
        let description = str_of(entry.get("description")).map(String::from);
        let context_windows = if one_m { context_windows() } else { vec![] };
        by_slug.insert(
            slug.clone(),
            HarnessCatalogModelProbe {
                slug,
                label,
                description,
                efforts: Some(efforts),
                default_effort: None,
                context_windows,
                is_default: false,
                fast: None,
            },
        );
    }
    if by_slug.is_empty() {
        return None;
    }
    let mut models: Vec<HarnessCatalogModelProbe> = by_slug.into_values().collect();
    for model in &mut models {
        model.is_default = default_slug.as_deref() == Some(model.slug.as_str());
    }
    Some(models)
}

/// The label less a trailing ` (1M context)`, case-insensitive, with the white space before it.
fn without_one_m_context(label: &str) -> &str {
    const TAIL: &str = "(1M context)";
    let cut = label.len().checked_sub(TAIL.len());
    match cut {
        Some(at) if label.is_char_boundary(at) && label[at..].eq_ignore_ascii_case(TAIL) => {
            label[..at].trim_end_matches(wsp_words::js::is_js_whitespace)
        }
        _ => label,
    }
}

/// The quoted choices after `flag` in the help text: the first `(choices: ...)` within 600 UTF-16 units of it.
fn quoted_list(help: &str, flag: &str) -> Vec<String> {
    let Some(at) = help.find(flag) else { return vec![] };
    let window = utf16::slice(&help[at..], 0, Some(600));
    let Some(choices) = Regex::new(r"\(choices:\s*([^)]*)\)").ok().and_then(|re| re.captures(&window)) else { return vec![] };
    let quoted = Regex::new(r#""([^"]+)""#).expect("a constant pattern");
    quoted.captures_iter(&choices[1]).map(|found| found[1].to_string()).collect()
}

/// The effort levels after `--effort <level>`: the first `(...)` within 300 UTF-16 units of the flag, lower-case words.
fn effort_choices(help: &str) -> Vec<String> {
    let Some(at) = help.find("--effort <level>") else { return vec![] };
    let window = utf16::slice(&help[at..], 0, Some(300));
    let Some(parens) = Regex::new(r"\(([^)]*)\)").ok().and_then(|re| re.captures(&window)) else { return vec![] };
    parens[1]
        .split(',')
        .map(js_trim)
        .filter(|word| !word.is_empty() && word.bytes().all(|b| b.is_ascii_lowercase()))
        .map(String::from)
        .collect()
}

/// catalog.ts:143. `None` without an initialize response naming a model.
pub fn parse_catalog_probe(stdout: &str) -> Option<HarnessCatalogProbe> {
    let parts: Vec<&str> = stdout.split(SEP).collect();
    if parts.len() < 3 {
        return None;
    }
    let (version_part, help, init_part) = (parts[0], parts[1], parts[2]);
    let Value::Object(init) = init_response(init_part)? else { return None };
    let models = models_of(&init)?;
    let mut permission_modes = vec!["default".to_string()];
    permission_modes.extend(quoted_list(help, "--permission-mode <mode>").into_iter().filter(|mode| mode != "default" && mode != "plan"));
    Some(HarnessCatalogProbe { version: parse_version(version_part), models, efforts: effort_choices(help), permission_modes })
}
