// SPDX-License-Identifier: AGPL-3.0-only
//! package.json: the manager it pins, the runtimes its engines field accepts, the programs its scripts run and the
//! packages it depends on. Port of packages/collect/src/project/readers/package-json.ts at a3b870a6e; the spec is
//! spec/project.md.
use indexmap::IndexMap;
use serde_json::Value;
use wsp_catalog::roads::RoadName;
use wsp_wire::wire::helpers::js_key_order;

use crate::history::commands::command_names;
use crate::project::reader::{ProjectFile, ProjectFinding, ProjectReader, Reads};

pub const FILES: [&str; 3] = ["package.json", "apps/*/package.json", "packages/*/package.json"];

/// The string-valued entries of an object (its keys in JavaScript's order) or the string items of an array (by index).
fn strings(value: Option<&Value>) -> Vec<(String, String)> {
    let entries: Vec<(String, Value)> = match value {
        Some(Value::Object(map)) => {
            let mut ordered: IndexMap<String, Value> = map.iter().map(|(k, v)| (k.clone(), v.clone())).collect();
            js_key_order(&mut ordered);
            ordered.into_iter().collect()
        }
        Some(Value::Array(items)) => items.iter().enumerate().map(|(i, v)| (i.to_string(), v.clone())).collect(),
        _ => return Vec::new(),
    };
    entries
        .into_iter()
        .filter_map(|(key, v)| match v {
            Value::String(s) => Some((key, s)),
            _ => None,
        })
        .collect()
}

/// The package.json's own findings: its manager, its engines, its scripts' programs and its dependencies. A text that
/// is not JSON, or a root that is not an object, gives nothing.
pub fn read(file: &ProjectFile) -> Vec<ProjectFinding> {
    let Ok(Value::Object(pkg)) = serde_json::from_str::<Value>(&file.text) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    if let Some(manager) = pkg.get("packageManager").and_then(Value::as_str).filter(|m| !m.is_empty()) {
        // corepack writes a `+sha512.<88 chars>` integrity suffix after the version; the line the row shows drops it.
        let spec = manager.split('+').next().unwrap_or(manager);
        let name = spec.split('@').next().unwrap_or(spec);
        if !name.is_empty() {
            out.push(ProjectFinding {
                name: name.to_string(),
                label: name.to_string(),
                why: format!("packageManager {spec}"),
                catalog_only: false,
                installs: None,
            });
        }
    }
    for (key, range) in strings(pkg.get("engines")) {
        out.push(ProjectFinding {
            name: key.clone(),
            label: key.clone(),
            why: format!("engines.{key} {range}"),
            catalog_only: true,
            installs: None,
        });
    }
    for (_, command) in strings(pkg.get("scripts")) {
        for name in command_names(&command) {
            out.push(ProjectFinding {
                name: name.clone(),
                label: name.clone(),
                why: format!("{} scripts run {name}", file.path),
                catalog_only: true,
                installs: None,
            });
        }
    }
    let dependencies = strings(pkg.get("dependencies")).into_iter().chain(strings(pkg.get("devDependencies")));
    for (name, range) in dependencies {
        out.push(ProjectFinding {
            name: name.clone(),
            label: name.clone(),
            why: format!("{} depends on {name} {range}", file.path),
            catalog_only: true,
            installs: Some(RoadName::Npm),
        });
    }
    out
}

pub const PACKAGE_JSON_READER: ProjectReader = ProjectReader { id: "package-json", files: &FILES, reads: Reads::Text, read };
