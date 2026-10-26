// SPDX-License-Identifier: AGPL-3.0-only
//! What the installed codex reports about itself, without a turn. TS adapter-codex/src/catalog.ts. Spec:
//! spec/codex_catalog.md.

use std::cmp::Ordering;

use regex::Regex;
use serde_json::{json, Map, Value};

use super::command::{codex_word, AccountModels, CodexBroker};
use super::fields::rec;
use crate::pending::{
    answers_of, app_server_initialize, app_server_request, app_server_script, codex_not_signed_in_line, result_of, ServerStage,
    CODEX_PINNED_VERSION,
};
use crate::port::{AgentLaunch, HarnessCatalogAnswer, HarnessCatalogModelProbe, HarnessCatalogProbe};
use wsp_wire::types::shell_quote::ENV_FROM_INPUT;
use wsp_words::js::{js_trim, utf16};

/// TS catalog.ts:354.
pub const SEP: &str = "__WSP_CATALOG_SEP__";

const INIT: i64 = 1;
const MODELS: i64 = 2;
const CONFIG: i64 = 3;
const ACCOUNT: i64 = 4;

/// A brokered machine's door holds the sign-in, so its account/read would always say none (catalog.ts:22-29).
fn probe_lines(broker: Option<&CodexBroker>) -> Vec<String> {
    let mut lines = vec![
        app_server_initialize(INIT),
        app_server_request(MODELS, "model/list", Map::new()),
        app_server_request(CONFIG, "config/read", Map::new()),
    ];
    if broker.is_none() {
        lines.push(app_server_request(ACCOUNT, "account/read", Map::new()));
    }
    lines
}

/// TS catalog.ts:373.
pub fn catalog_probe_command(launch: Option<&AgentLaunch>, broker: Option<&CodexBroker>) -> Result<String, String> {
    let codex = codex_word(launch, broker)?;
    let lines = probe_lines(broker);
    let answers = lines.len() as u32;
    let server = app_server_script(&codex, &[ServerStage { lines, answers }]);
    Ok(format!("{}; echo {SEP}; {codex} --help; echo {SEP}\n{server}", version_probe_command(launch, broker)?))
}

/// TS catalog.ts:382.
pub fn version_probe_command(launch: Option<&AgentLaunch>, broker: Option<&CodexBroker>) -> Result<String, String> {
    Ok(format!("cd ~ && {ENV_FROM_INPUT}; {} --version", codex_word(launch, broker)?))
}

/// The first `\d+\.\d+\.\d+` in the text, ASCII digits only. TS catalog.ts:387.
pub fn parse_version(stdout: &str) -> Option<String> {
    let bytes = stdout.as_bytes();
    (0..bytes.len()).find_map(|start| {
        if !bytes[start].is_ascii_digit() {
            return None;
        }
        let mut end = start;
        for part in 0..3 {
            let digits = end;
            while end < bytes.len() && bytes[end].is_ascii_digit() {
                end += 1;
            }
            if end == digits {
                return None;
            }
            if part < 2 {
                if bytes.get(end) != Some(&b'.') {
                    return None;
                }
                end += 1;
            }
        }
        Some(stdout[start..end].to_string())
    })
}

/// The `[possible values: ...]` list under the flag, within 600 units of it. TS catalog.ts:53.
fn possible_values(help: &str, flag: &str) -> Vec<String> {
    let Some(at) = help.find(flag) else {
        return Vec::new();
    };
    let window = utf16::cut(&help[at..], 600);
    let pattern = Regex::new(r"\[possible values:\s*([^\]]*)\]").expect("a fixed pattern");
    let Some(values) = pattern.captures(&window) else {
        return Vec::new();
    };
    values[1].split(',').map(js_trim).filter(|s| !s.is_empty()).map(String::from).collect()
}

/// A string that is not empty (catalog.ts `str`, which refuses "").
fn nonempty(value: Option<&Value>) -> Option<&str> {
    value?.as_str().filter(|s| !s.is_empty())
}

fn field(row: &Map<String, Value>, key: &str) -> Value {
    row.get(key).cloned().unwrap_or(Value::Null)
}

fn num(value: Option<&Value>) -> f64 {
    value.and_then(Value::as_f64).filter(|n| n.is_finite()).unwrap_or(0.0)
}

/// ChatGPT's /codex/models answer as model/list rows (catalog.ts:445): sorted by priority, the first row listed
/// is the default. TS catalog.ts:445.
pub fn account_rows(answer: Option<&Value>) -> Option<Map<String, Value>> {
    let raw = rec(answer)?.get("models")?.as_array()?;
    let mut rows: Vec<&Map<String, Value>> =
        raw.iter().filter_map(|m| rec(Some(m))).filter(|m| nonempty(m.get("slug")).is_some()).collect();
    rows.sort_by(|a, b| num(a.get("priority")).partial_cmp(&num(b.get("priority"))).unwrap_or(Ordering::Equal));
    let first = rows.iter().position(|m| m.get("visibility").and_then(Value::as_str) == Some("list"))?;
    let data: Vec<Value> = rows
        .iter()
        .enumerate()
        .map(|(i, m)| {
            let efforts: Vec<Value> = m
                .get("supported_reasoning_levels")
                .and_then(Value::as_array)
                .map(|levels| {
                    levels.iter().map(|l| json!({"reasoningEffort": rec(Some(l)).map_or(Value::Null, |o| field(o, "effort"))})).collect()
                })
                .unwrap_or_default();
            json!({
                "id": field(m, "slug"),
                "displayName": field(m, "display_name"),
                "description": field(m, "description"),
                "supportedReasoningEfforts": efforts,
                "defaultReasoningEffort": field(m, "default_reasoning_level"),
                "additionalSpeedTiers": field(m, "additional_speed_tiers"),
                "hidden": m.get("visibility").and_then(Value::as_str) != Some("list"),
                "isDefault": i == first,
            })
        })
        .collect();
    Some(Map::from_iter([("data".to_string(), Value::Array(data))]))
}

/// The models model/list offers, the hidden ones left out (catalog.ts:71). TS catalog.ts:71.
fn models_of(listed: Option<&Map<String, Value>>, configured: Option<&str>) -> Vec<HarnessCatalogModelProbe> {
    let rows = listed.and_then(|l| l.get("data")).and_then(Value::as_array).map(Vec::as_slice).unwrap_or_default();
    let mut models = Vec::new();
    for raw in rows {
        let Some(entry) = rec(Some(raw)) else {
            continue;
        };
        let Some(slug) = nonempty(entry.get("id")) else {
            continue;
        };
        if entry.get("hidden") == Some(&Value::Bool(true)) {
            continue;
        }
        let efforts: Vec<String> = entry
            .get("supportedReasoningEfforts")
            .and_then(Value::as_array)
            .map(|list| {
                list.iter().filter_map(|e| nonempty(rec(Some(e)).and_then(|o| o.get("reasoningEffort")))).map(String::from).collect()
            })
            .unwrap_or_default();
        let is_default = match configured {
            None => entry.get("isDefault") == Some(&Value::Bool(true)),
            Some(c) => slug == c,
        };
        let fast = entry
            .get("additionalSpeedTiers")
            .and_then(Value::as_array)
            .is_some_and(|tiers| tiers.iter().any(|t| t.as_str() == Some("fast")));
        models.push(HarnessCatalogModelProbe {
            slug: slug.to_string(),
            label: nonempty(entry.get("displayName")).unwrap_or(slug).to_string(),
            description: nonempty(entry.get("description")).map(String::from),
            efforts: Some(efforts),
            default_effort: nonempty(entry.get("defaultReasoningEffort")).map(String::from),
            context_windows: Vec::new(),
            is_default,
            fast: fast.then_some(true),
        });
    }
    if let Some(c) = configured {
        if !models.iter().any(|m| m.slug == c) {
            models.insert(
                0,
                HarnessCatalogModelProbe {
                    slug: c.into(),
                    label: c.into(),
                    description: None,
                    efforts: None,
                    default_effort: None,
                    context_windows: Vec::new(),
                    is_default: true,
                    fast: None,
                },
            );
        }
    }
    models
}

/// The efforts the listed models take between them, first-seen order (catalog.ts:128). TS catalog.ts:128.
fn efforts_of(models: &[HarnessCatalogModelProbe]) -> Vec<String> {
    let mut efforts: Vec<String> = Vec::new();
    for effort in models.iter().flat_map(|m| m.efforts.iter().flatten()) {
        if !efforts.contains(effort) {
            efforts.push(effort.clone());
        }
    }
    efforts
}

/// TS catalog.ts:476.
pub fn parse_catalog_probe(stdout: &str, login: &str, account_list: Option<&Value>) -> Option<HarnessCatalogAnswer> {
    let parts: Vec<&str> = stdout.split(SEP).collect();
    if parts.len() < 3 {
        return None;
    }
    let (version_part, help, server_part) = (parts[0], parts[1], parts[2]);
    let answers = answers_of(server_part);
    result_of(&answers, INIT)?;
    let config = result_of(&answers, CONFIG).and_then(|r| rec(r.get("config")));
    let configured = nonempty(config.and_then(|c| c.get("model")));
    let listed = account_rows(account_list).or_else(|| result_of(&answers, MODELS).cloned());
    let models = models_of(listed.as_ref(), configured);
    if models.is_empty() {
        let account = result_of(&answers, ACCOUNT);
        let wants_sign_in =
            account.is_some_and(|a| a.get("requiresOpenaiAuth") == Some(&Value::Bool(true)) && a.get("account") == Some(&Value::Null));
        return wants_sign_in.then(|| HarnessCatalogAnswer::Refused(codex_not_signed_in_line(login)));
    }
    Some(HarnessCatalogAnswer::Probe(HarnessCatalogProbe {
        version: parse_version(version_part),
        efforts: efforts_of(&models),
        permission_modes: possible_values(help, "--sandbox <SANDBOX_MODE>"),
        models,
    }))
}

/// TS catalog.ts:500.
pub async fn read_catalog_probe(stdout: &str, login: &str, models: Option<&AccountModels>) -> Option<HarnessCatalogAnswer> {
    let version = parse_version(stdout.split(SEP).next().unwrap_or(""));
    let account_list = match (models, version) {
        (Some(read), Some(version)) => read.as_ref()(version).await.ok().flatten(),
        _ => None,
    };
    parse_catalog_probe(stdout, login, account_list.as_ref())
}

/// The account's models where no binary has described itself, for `version`, else wsp's npm pin. TS catalog.ts:509.
pub async fn account_models_of(models: &AccountModels, version: Option<&str>) -> Option<Vec<HarnessCatalogModelProbe>> {
    let list = models.as_ref()(version.unwrap_or(CODEX_PINNED_VERSION).to_string()).await.ok().flatten();
    let rows = account_rows(list.as_ref())?;
    Some(models_of(Some(&rows), None))
}
