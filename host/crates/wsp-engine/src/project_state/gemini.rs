// SPDX-License-Identifier: AGPL-3.0-only
//! Port of packages/engine/src/project-state/gemini.ts. Spec: spec/project_state_agents.md.
use std::path::Path;

use async_trait::async_trait;
use serde_json::{json, Map, Value};

use wsp_catalog::config_write::config_sum;
use wsp_wire::views::project_bundle::ProjectCarry;
use wsp_words::js::js_trim_end;
use wsp_words::js::number_string;
use wsp_words::js::utf16::cmp as js_cmp;
use wsp_words::project_path::under_project;

use crate::config_here::write_config_here;
use crate::errors::{EngineError, Result};
use crate::project_state::merge::merge_script;
use crate::project_state::py::py_data;
use crate::project_state::resolver::{
    files_under, join_posix, js_read, js_write, moved_path, read_bytes, under_home, MovedState, ProjectStateResolver, SharedStore,
};

/// gemini.ts:16.
pub const REGISTRY: &str = "projects.json";
/// gemini.ts:23. The catalog rows a slug names, each with its directory under the home.
pub const SLUG_DIRS: [(&str, &str); 2] = [("project temp dir", "tmp"), ("shell history", "history")];

/// gemini.ts:26. carry transcript-only; states project registry, project temp dir, shell history; roots projects.json, tmp, history; shared store the registry filtered to keys under PATH.
pub struct GeminiResolver;

fn other(message: impl Into<String>) -> EngineError {
    EngineError::Other { message: message.into(), code: None }
}

fn read_registry(file: &str) -> Result<Option<Value>> {
    if !Path::new(file).exists() {
        return Ok(None);
    }
    let bytes = read_bytes(file)?;
    js_read(&String::from_utf8_lossy(&bytes)).map(Some).ok_or_else(|| other(format!("{file} is not JSON")))
}

/// A non-object `projects` has no key under an absolute path, so it lists nothing.
fn project_entries(registry: &Value) -> Vec<(&String, &Value)> {
    match registry.get("projects") {
        Some(Value::Object(map)) => map.iter().collect(),
        _ => vec![],
    }
}

/// `project_entries` of a registry the caller is done with, moved out so no slug is cloned.
fn taken_entries(registry: Value) -> Vec<(String, Value)> {
    match registry {
        Value::Object(mut root) => match root.remove("projects") {
            Some(Value::Object(map)) => map.into_iter().collect(),
            _ => vec![],
        },
        _ => vec![],
    }
}

fn string_slug(slug: &Value) -> Result<String> {
    match slug {
        Value::String(s) => Ok(s.clone()),
        Value::Null => Err(node_path_error("Received null".into())),
        Value::Bool(b) => Err(node_path_error(format!("Received type boolean ({b})"))),
        Value::Number(n) => Err(node_path_error(format!("Received type number ({})", number_string(n.as_f64().unwrap_or(f64::NAN))))),
        Value::Array(_) => Err(node_path_error("Received an instance of Array".into())),
        Value::Object(_) => Err(node_path_error("Received an instance of Object".into())),
    }
}

fn node_path_error(received: String) -> EngineError {
    EngineError::Other {
        message: format!("The \"path\" argument must be of type string. {received}"),
        code: Some("ERR_INVALID_ARG_TYPE".into()),
    }
}

/// A slug that climbs out of the home is dropped: the registry is the machine's own bytes.
fn slugs_under(home: &str, path: &str) -> Result<Vec<String>> {
    let Some(registry) = read_registry(&join_posix(&[home, REGISTRY]))? else {
        return Ok(vec![]);
    };
    let mut slugs = Vec::new();
    for (key, slug) in project_entries(&registry) {
        if !under_project(key, path) {
            continue;
        }
        let slug = string_slug(slug)?;
        if under_home(home, &[slug.as_str()]).is_some() {
            slugs.push(slug);
        }
    }
    Ok(slugs)
}

#[async_trait(?Send)]
impl ProjectStateResolver for GeminiResolver {
    fn agent(&self) -> &'static str {
        "gemini"
    }
    fn states(&self) -> Vec<&'static str> {
        vec!["project registry", "project temp dir", "shell history"]
    }
    fn roots(&self) -> Vec<&'static str> {
        vec![REGISTRY, "tmp", "history"]
    }
    fn carry(&self) -> ProjectCarry {
        ProjectCarry::TranscriptOnly
    }
    async fn move_state(&self, home: &str, from: &str, to: &str) -> Result<Vec<MovedState>> {
        let file = join_posix(&[home, REGISTRY]);
        if !Path::new(&file).exists() {
            return Ok(vec![]);
        }
        let bytes = read_bytes(&file)?;
        let mut registry = js_read(&String::from_utf8_lossy(&bytes)).ok_or_else(|| other(format!("{file} is not JSON")))?;
        if registry.is_null() {
            return Err(other("Cannot read properties of null (reading 'projects')"));
        }
        let projects = match registry.get("projects") {
            Some(Value::Null) | None => return Err(other("Cannot convert undefined or null to object")),
            Some(projects) => projects,
        };
        let mut renamed: Vec<(String, String, String)> = Vec::new();
        for (path, slug) in project_entries(&registry) {
            let Some(target) = moved_path(path, from, to) else {
                continue;
            };
            let slug = string_slug(slug)?;
            if under_home(home, &[slug.as_str()]).is_none() {
                continue;
            }
            if matches!(projects, Value::Object(map) if map.contains_key(&target)) {
                return Err(other(format!("{target} already exists in {file}")));
            }
            renamed.push((path.clone(), slug, target));
        }
        if renamed.is_empty() {
            return Ok(vec![]);
        }
        if let Value::Object(root) = &mut registry {
            let mut next = Map::new();
            if let Some(Value::Object(held)) = root.get_mut("projects") {
                for (key, slug) in std::mem::take(held) {
                    let key = match renamed.iter().find(|r| r.0 == key) {
                        Some(r) => r.2.clone(),
                        None => key,
                    };
                    next.insert(key, slug);
                }
            }
            root.insert("projects".into(), Value::Object(next));
        }
        let sum = config_sum(&bytes);
        write_config_here(&file, home, Some(sum.as_str()), &(js_write(&registry, Some(2)) + "\n"), None)?;
        let mut moved =
            vec![MovedState { state: "project registry".into(), files: vec![file.clone()], changed: renamed.len() as u64, skipped: None }];
        for (state, dir) in SLUG_DIRS {
            let mut files = Vec::new();
            for (path, slug, target) in &renamed {
                let marker = join_posix(&[home, dir, slug.as_str(), ".project_root"]);
                if !Path::new(&marker).exists() {
                    continue;
                }
                let marker_bytes = read_bytes(&marker)?;
                let text = String::from_utf8_lossy(&marker_bytes).into_owned();
                if js_trim_end(&text) != path.as_str() {
                    continue;
                }
                let sum = config_sum(&marker_bytes);
                write_config_here(&marker, home, Some(sum.as_str()), &text.replacen(path.as_str(), target.as_str(), 1), None)?;
                files.push(marker);
            }
            if !files.is_empty() {
                let changed = files.len() as u64;
                moved.push(MovedState { state: state.into(), files, changed, skipped: None });
            }
        }
        Ok(moved)
    }
    async fn sessions(&self, home: &str, path: &str) -> Result<u64> {
        let mut n = 0;
        for slug in slugs_under(home, path)? {
            n += files_under(&join_posix(&[home, "tmp", slug.as_str(), "chats"]), ".jsonl")?.len() as u64;
        }
        Ok(n)
    }
    async fn entries(&self, home: &str, path: &str) -> Result<Vec<String>> {
        let mut out = Vec::new();
        for slug in slugs_under(home, path)? {
            for (_, dir) in SLUG_DIRS {
                out.extend(files_under(&join_posix(&[home, dir, slug.as_str()]), "")?);
            }
        }
        out.sort_by(|a, b| js_cmp(a, b));
        Ok(out)
    }
    fn listing(&self, home: &str, _path: &str) -> Option<Vec<String>> {
        Some(vec![
            "with open(COPY, encoding=\"utf-8\") as h:".into(),
            "    slugs = json.load(h)[\"projects\"].values()".into(),
            "for slug in slugs:".into(),
            "    if not isinstance(slug, str):".into(),
            "        continue".into(),
            format!("    for d in {}:", py_data(&json!(SLUG_DIRS.map(|(_, dir)| dir)))),
            format!("        say(os.path.join({}, d, slug))", py_data(&json!(home))),
        ])
    }
    fn shared(&self) -> Option<SharedStore> {
        Some(SharedStore {
            store: REGISTRY.into(),
            filter: vec![
                "with open(STORE, encoding=\"utf-8\") as h:".into(),
                "    reg = json.load(h)".into(),
                "kept = {k: v for k, v in (reg.get(\"projects\") or {}).items() if under(k, PATH)}".into(),
                "reg[\"projects\"] = kept".into(),
                "FILTERED = len(kept)".into(),
                "with open(COPY, \"w\", encoding=\"utf-8\") as h:".into(),
                "    h.write(json.dumps(reg, indent=2, ensure_ascii=False) + \"\\n\")".into(),
            ],
        })
    }
    fn has_merge(&self) -> bool {
        true
    }
    async fn merge(&self, home: &str, from: &str, to: &str, guest_home: &str) -> Result<Option<String>> {
        let registry = read_registry(&join_posix(&[home, REGISTRY]))?.unwrap_or(Value::Null);
        let keys: Vec<Value> = taken_entries(registry)
            .into_iter()
            .filter_map(|(path, slug)| moved_path(&path, from, to).map(|target| Value::Array(vec![json!(path), json!(target), slug])))
            .collect();
        if keys.is_empty() {
            return Ok(None);
        }
        let dirs: Vec<&str> = SLUG_DIRS.iter().map(|(_, dir)| *dir).collect();
        let lines: Vec<String> = vec![
            format!("HOME = {}", py_data(&json!(guest_home))),
            format!("REG = {}", py_data(&json!(join_posix(&[guest_home, REGISTRY])))),
            format!("KEYS = {}", py_data(&Value::Array(keys))),
            format!("DIRS = {}", py_data(&json!(dirs))),
            "os.makedirs(HOME, exist_ok=True)".into(),
            "if os.path.exists(REG):".into(),
            "    with open(REG, encoding=\"utf-8\") as h:".into(),
            "        reg = json.load(h)".into(),
            "else:".into(),
            "    reg = {\"projects\": {}}".into(),
            "projects = reg.setdefault(\"projects\", {})".into(),
            "for old, key, slug in KEYS:".into(),
            "    for other, s in projects.items():".into(),
            "        if s == slug and other != key:".into(),
            "            fail(\"slug \" + slug + \" already belongs to \" + other + \" in \" + REG)".into(),
            "changed = False".into(),
            "for old, key, slug in KEYS:".into(),
            "    held = projects.get(key)".into(),
            "    if held == slug:".into(),
            "        kept += 1".into(),
            "    elif held is not None:".into(),
            "        kept += 1".into(),
            "        notes.append(\"the machine already lists \" + key + \" as \" + str(held) + \", so that slug stays and the carried chats under tmp/\" + slug + \" are not listed there\")".into(),
            "    else:".into(),
            "        projects[key] = slug".into(),
            "        merged += 1".into(),
            "        changed = True".into(),
            "    for d in DIRS:".into(),
            "        marker = os.path.join(HOME, d, slug, \".project_root\")".into(),
            "        if not os.path.exists(marker):".into(),
            "            continue".into(),
            "        with open(marker, encoding=\"utf-8\", newline=\"\") as h:".into(),
            "            text = h.read()".into(),
            "        if text.rstrip() == old:".into(),
            "            write(marker, key + text[len(text.rstrip()):])".into(),
            "if changed:".into(),
            "    write(REG, json.dumps(reg, indent=2, ensure_ascii=False) + \"\\n\")".into(),
        ];
        Ok(Some(merge_script(from, to, &[lines.join("\n")])))
    }
}
