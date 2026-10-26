// SPDX-License-Identifier: AGPL-3.0-only
//! What the installed OpenCode binary reports about itself. TS adapter-opencode/src/catalog.ts. Spec: spec/opencode.md
//! rules 15-17.

use indexmap::IndexMap;
use serde_json::Value;

use crate::opencode::command::AUTO_MODE;
use crate::pending::shell_quote;
use crate::port::{program_word, AgentLaunch, HarnessCatalogAnswer, HarnessCatalogModelProbe, HarnessCatalogProbe};
use wsp_words::js::{is_js_whitespace, js_key_order, js_trim};

/// TS catalog.ts:11.
pub const SEP: &str = "__WSP_CATALOG_SEP__";

/// TS catalog.ts:15.
pub fn catalog_probe_command(base_env: Option<&IndexMap<String, Option<String>>>, launch: Option<&AgentLaunch>) -> String {
    format!("{}; echo {SEP}; {} models --verbose", version_probe_command(base_env, launch), program_word("opencode", launch))
}

/// TS catalog.ts:22.
pub fn version_probe_command(base_env: Option<&IndexMap<String, Option<String>>>, launch: Option<&AgentLaunch>) -> String {
    let path = base_env.and_then(|env| env.get("PATH")).and_then(|value| value.as_deref());
    let export = path.map(|path| format!("export PATH={}; ", shell_quote(path))).unwrap_or_default();
    format!("cd ~ && {export}{} --version", program_word("opencode", launch))
}

/// TS catalog.ts:28.
pub fn parse_version(stdout: &str) -> Option<String> {
    let last = js_trim(js_trim(stdout).split('\n').last().unwrap_or(""));
    (!last.is_empty()).then(|| last.to_string())
}

/// `^[A-Za-z0-9][\w.-]*\/\S+$`: the head ends at the first slash, the tail is non-empty and has no JS white space.
fn is_model_line(line: &str) -> bool {
    match line.split_once('/') {
        Some((head, tail)) => {
            let mut chars = head.chars();
            matches!(chars.next(), Some(c) if c.is_ascii_alphanumeric())
                && chars.all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '.' | '-'))
                && !tail.is_empty()
                && !tail.chars().any(is_js_whitespace)
        }
        None => false,
    }
}

/// TS catalog.ts `modelOf`: the body's JSON read as one model, `None` where it is no JSON object.
fn model_of(slug: &str, body: &[String]) -> Option<HarnessCatalogModelProbe> {
    let Value::Object(entry) = wsp_words::v8_json::parse_json_lax(&body.join("\n"))? else {
        return None;
    };
    let label = match entry.get("name") {
        Some(Value::String(name)) if !name.is_empty() => name.clone(),
        _ => slug.to_string(),
    };
    let efforts = match entry.get("variants") {
        Some(Value::Object(variants)) => js_key_order(variants.iter()).into_iter().map(|(key, _)| key.clone()).collect(),
        _ => Vec::new(),
    };
    Some(HarnessCatalogModelProbe {
        slug: slug.to_string(),
        label,
        description: None,
        efforts: Some(efforts),
        default_effort: None,
        context_windows: Vec::new(),
        is_default: false,
        fast: None,
    })
}

/// TS catalog.ts:46. `None` where no model was printed.
pub fn parse_catalog_probe(stdout: &str) -> Option<HarnessCatalogAnswer> {
    let at = stdout.find(SEP)?;
    let version = parse_version(&stdout[..at]);
    let mut models: Vec<HarnessCatalogModelProbe> = Vec::new();
    let mut slug: Option<String> = None;
    let mut body: Vec<String> = Vec::new();
    for line in stdout[at + SEP.len()..].split('\n') {
        if !is_model_line(line) {
            body.push(line.to_string());
            continue;
        }
        if let Some(model) = slug.as_deref().and_then(|slug| model_of(slug, &body)) {
            models.push(model);
        }
        slug = Some(line.to_string());
        body.clear();
    }
    if let Some(model) = slug.as_deref().and_then(|slug| model_of(slug, &body)) {
        models.push(model);
    }
    if models.is_empty() {
        return None;
    }
    let mut efforts: Vec<String> = Vec::new();
    for effort in models.iter().flat_map(|model| model.efforts.iter().flatten()) {
        if !efforts.contains(effort) {
            efforts.push(effort.clone());
        }
    }
    Some(HarnessCatalogAnswer::Probe(HarnessCatalogProbe { version, models, efforts, permission_modes: vec![AUTO_MODE.to_string()] }))
}
