// SPDX-License-Identifier: AGPL-3.0-only
//! The wave-2 oracle fixtures (tests/fixtures/oracle/w2/<unit>.json, written by oracle/w2-*.oracle.ts) and the laptop
//! each case describes, rebuilt as the TS oracle built it (oracle/w2-host.ts).
#![allow(dead_code)]
use serde_json::Value;

use super::fake_host::{fake_host, FakeHost, FakeLaptop};
use wsp_collect::host::Platform;

pub fn w2(unit: &str) -> Value {
    let path = format!("{}/tests/fixtures/oracle/w2/{unit}.json", env!("CARGO_MANIFEST_DIR"));
    serde_json::from_str(&std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{path}: {e}"))).unwrap()
}

/// The cases recorded under `key`, each an object with at least `name`.
pub fn cases<'a>(fixture: &'a Value, key: &str) -> &'a Vec<Value> {
    fixture[key].as_array().unwrap_or_else(|| panic!("no cases under {key}"))
}

fn strings(v: &Value) -> Vec<String> {
    v.as_array().map(|a| a.iter().map(|s| s.as_str().unwrap().to_string()).collect()).unwrap_or_default()
}

fn string_map(v: &Value) -> Option<indexmap::IndexMap<String, String>> {
    v.as_object().map(|o| o.iter().map(|(k, v)| (k.clone(), v.as_str().unwrap().to_string())).collect())
}

/// oracle/w2-host.ts's `Laptop` as the Rust fake: files with a number are sizes, with a string their text.
pub fn laptop(l: &Value) -> FakeLaptop {
    let mut out = FakeLaptop::new();
    match l["platform"].as_str() {
        Some("linux") => out = out.platform(Platform::Linux),
        Some("darwin") => out = out.platform(Platform::Darwin),
        _ => {}
    }
    for (path, v) in l["files"].as_object().into_iter().flatten() {
        out = match v {
            Value::String(t) => out.file(path, t.as_str()),
            n => out.file(path, n.as_u64().unwrap_or_else(|| panic!("size of {path}"))),
        };
    }
    out = out.which(&strings(&l["which"]).iter().map(String::as_str).collect::<Vec<_>>());
    for (key, v) in l["exec"].as_object().into_iter().flatten() {
        out = out.exec(key, v.as_str().unwrap());
    }
    for (path, v) in l["mtimes"].as_object().into_iter().flatten() {
        out = out.mtime(path, v.as_f64().unwrap());
    }
    if let Some(s) = l["shell"].as_str() {
        out = out.shell(s);
    }
    if let Some(t) = l["terminal"].as_str() {
        out = out.terminal(t);
    }
    if let Some(x) = l["xdgConfigHome"].as_str() {
        out = out.xdg_config_home(x);
    }
    out
}

/// The laptop's fake host, with the fields FakeLaptop has no builder for set on the host.
pub fn host(l: &Value) -> FakeHost {
    let mut h = fake_host(laptop(l));
    h.host.claude_config_dir = l["claudeConfigDir"].as_str().map(str::to_string);
    h.host.codex_home = l["codexHome"].as_str().map(str::to_string);
    h.host.stores = string_map(&l["stores"]);
    h.host.seeds = string_map(&l["seeds"]);
    h
}

pub fn strs(v: &Value) -> Vec<String> {
    strings(v)
}
