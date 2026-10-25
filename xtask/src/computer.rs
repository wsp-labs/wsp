// SPDX-License-Identifier: AGPL-3.0-only
//! Tapes whose answers read the computer the host runs on: the tools it has installed, its cores, the user it runs as,
//! where node is. Two runs on one computer agree on them, so the normalizer leaves them in; another computer answers
//! them otherwise, so `record` names the computer in the tape's meta (`"computer"`), a check elsewhere skips the tape
//! and says so, and a record elsewhere leaves its golden alone unless told to move it.

use crate::tape::{Door, Tape};
use serde_json::{Map, Value};
use std::fs;
use std::path::Path;
use std::process::Command;
use std::sync::OnceLock;

/// This computer's name, as its host name says it, up to the first dot.
pub fn name() -> String {
    static NAME: OnceLock<String> = OnceLock::new();
    NAME.get_or_init(|| {
        let full = fs::read_to_string("/proc/sys/kernel/hostname")
            .ok()
            .or_else(|| Command::new("hostname").output().ok().map(|o| String::from_utf8_lossy(&o.stdout).into_owned()))
            .unwrap_or_default();
        full.trim().split('.').next().unwrap_or_default().to_string()
    })
    .clone()
}

/// Keys with a value that reads what the computer holds, the value's start written with them: a tool's input schema
/// names the same keys with an object after them.
const KEYS: &[&str] = &[
    "\"installed\":[",
    "\"installed\":true",
    "\"installed\":false",
    "\"engine\":\"",
    "\"editors\":[",
    "\"user\":\"",
    "\"cores\":",
    "\"capDefault\":{",
];
/// Words an answer prints only from what this computer holds: the recipe's table of what is installed here, with sizes.
const WORDS: &[&str] = &["wsp-daemon binary missing", "ExperimentalWarning", "CORES", "installed here"];
/// Command lines and tools whose whole answer is a scan of the computer.
const SCANS: &[&str] = &["doctor", "setup", "recipe scan", "recipe_scan", "init.get", "agents.here"];

/// Whether a tape's golden reads the computer the host ran on.
pub fn reads(tape: &Tape, golden: &Value) -> bool {
    let text = golden.to_string();
    let scan = match tape.door {
        Door::Ws => tape.ops().iter().any(|op| SCANS.contains(&op.as_str())),
        Door::Mcp => tape.ins.iter().filter_map(|f| f.pointer("/params/name").and_then(Value::as_str)).any(|t| SCANS.contains(&t)),
        Door::Cli => tape.ins.iter().any(|f| {
            let words: Vec<&str> = f
                .get("argv")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .filter_map(Value::as_str)
                .filter(|w| !w.starts_with('-'))
                .collect();
            SCANS.iter().any(|s| words.join(" ").starts_with(s))
        }),
    };
    scan || KEYS.iter().chain(WORDS).any(|k| text.contains(k)) || node().is_some_and(|n| text.contains(n))
}

/// Where node is on this computer, which a `wsp mcp install` line prints.
fn node() -> Option<&'static str> {
    static NODE: OnceLock<Option<String>> = OnceLock::new();
    NODE.get_or_init(|| {
        let path = std::env::var("PATH").unwrap_or_default();
        path.split(':').map(|d| Path::new(d).join("node")).find(|p| p.is_file()).map(|p| p.display().to_string())
    })
    .as_deref()
}

/// Names the computer a tape was recorded on in its meta line, or takes the name out; every other line stays as it was.
pub fn mark(file: &Path, computer: Option<&str>) -> Result<(), String> {
    let text = fs::read_to_string(file).map_err(|e| format!("{}: {e}", file.display()))?;
    let (first, rest) = text.split_once('\n').unwrap_or((&text, ""));
    let parsed: Option<Map<String, Value>> =
        serde_json::from_str::<Value>(first).ok().and_then(|v| v.get("meta").and_then(Value::as_object).cloned());
    let (mut meta, body) = match parsed {
        Some(meta) => (meta, rest.to_string()),
        None => (Map::new(), text.clone()),
    };
    match computer {
        Some(c) => {
            meta.insert("computer".into(), Value::String(c.to_string()));
        }
        None => {
            meta.remove("computer");
        }
    }
    let line = serde_json::json!({ "meta": meta }).to_string();
    fs::write(file, format!("{line}\n{body}")).map_err(|e| format!("{}: {e}", file.display()))
}
