// SPDX-License-Identifier: AGPL-3.0-only
//! Port of packages/engine/src/project-state/listing.ts. Spec: spec/project_state_scripts.md.
//!
//! Which paths under an agent's home on the machine hold one project's state: a python3 script the machine runs, so a
//! trip pulls only the paths it prints, and a store kept for every project travels as a filtered copy.
use base64::Engine;
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::errors::{EngineError, Result};
use crate::project_state::py::{py_data, PY_PREAMBLE};
use crate::project_state::resolver::{join_posix, SharedStore};

/// listing.ts:16. The step that names a module's whole roots.
pub const ROOTS_STEP: &str = "for r in ROOTS:\n    say(r)";

/// listing.ts:22. The mark a listing prints an unreadable store on, ahead of its base64 JSON.
pub const UNREAD_MARK: &str = "!unread ";

/// listing.ts:26. Keys in this order on the wire: agent, store, why.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UnreadStore {
    pub agent: String,
    pub store: String,
    pub why: String,
}

/// listing.ts:33 answer.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StateListing {
    pub paths: Vec<String>,
    pub unread: Vec<UnreadStore>,
}

/// listing.ts:33. Each non-empty line of stdout split on `\n`: a line opening with `UNREAD_MARK` is the base64 JSON of
/// an UnreadStore, any other a path, both in order. A marked line that does not decode or parse is a failure (the TS
/// `JSON.parse` throws).
pub fn parse_state_listing(stdout: &str) -> Result<StateListing> {
    let mut listing = StateListing::default();
    for line in stdout.split('\n').filter(|l| !l.is_empty()) {
        match line.strip_prefix(UNREAD_MARK) {
            Some(encoded) => {
                let bytes = base64::engine::general_purpose::STANDARD
                    .decode(encoded)
                    .map_err(|e| EngineError::Other { message: e.to_string(), code: None })?;
                let store = serde_json::from_slice(&bytes).map_err(|e| EngineError::Other { message: e.to_string(), code: None })?;
                listing.unread.push(store);
            }
            None => listing.paths.push(line.to_string()),
        }
    }
    Ok(listing)
}

/// listing.ts:50. The whole listing script for one agent's home, byte for byte as the TS builds it (pinned by the
/// oracle): preamble, AGENT, HOME, PATH normalised, ROOTS, OUT, UNREAD, say(), unread(), the steps indented four spaces
/// inside `try:`, the roots fallback, the prints.
pub fn list_script(agent: &str, home: &str, path: &str, roots: &[String], steps: &[String]) -> String {
    let mut lines: Vec<String> = vec![
        PY_PREAMBLE.to_string(),
        format!("AGENT = {}", py_data(&json!(agent))),
        format!("HOME = {}", py_data(&json!(home))),
        format!("PATH = os.path.normpath({})", py_data(&json!(path))),
        format!("ROOTS = [os.path.join(HOME, r) for r in {}]", py_data(&json!(roots))),
        "OUT = []".into(),
        "UNREAD = []".into(),
        "def say(p):".into(),
        "    if os.path.exists(p) and p not in OUT:".into(),
        "        OUT.append(p)".into(),
        "def unread(store, copy, why, said):".into(),
        "    del OUT[said:]".into(),
        "    if os.path.exists(copy):".into(),
        "        os.remove(copy)".into(),
        "    UNREAD.append({\"agent\": AGENT, \"store\": store, \"why\": str(why) or type(why).__name__})".into(),
        "try:".into(),
    ];
    lines.extend(steps.iter().flat_map(|s| indent_lines(s, 4)));
    lines.push("except Exception:".into());
    lines.push("    OUT = []".into());
    lines.extend(indent_lines(ROOTS_STEP, 4));
    lines.push("for p in OUT:".into());
    lines.push("    print(p)".into());
    lines.push("for u in UNREAD:".into());
    lines
        .push(format!("    print({} + base64.b64encode(json.dumps(u).encode(\"utf-8\")).decode(\"ascii\"))", py_data(&json!(UNREAD_MARK))));
    lines.join("\n")
}

/// listing.ts:88. The keyed-directories step, `key` a python expression for the module's key rule.
pub fn keyed_step(parent: &str, key: &str) -> String {
    let mut lines: Vec<String> = vec![format!("PARENT = {}", py_data(&json!(parent))), format!("key = {key}")];
    lines.extend(
        [
            "def recorded(d):",
            "    found = []",
            "    for base, _dirs, names in os.walk(d):",
            "        found += [os.path.join(base, n) for n in names if n.endswith(\".jsonl\")]",
            "    for f in sorted(found):",
            "        with open(f, encoding=\"utf-8\", errors=\"surrogateescape\", newline=\"\") as h:",
            "            for line in h:",
            "                try:",
            "                    obj = json.loads(line)",
            "                except ValueError:",
            "                    continue",
            "                if isinstance(obj, dict) and isinstance(obj.get(\"cwd\"), str):",
            "                    return obj[\"cwd\"]",
            "    return None",
            "own = key(PATH)",
            "other = key(PATH + \"/x\")",
            "n = 0",
            "while n < len(own) and n < len(other) and own[n] == other[n]:",
            "    n += 1",
            "stem = own[:n]",
            "for name in sorted(os.listdir(PARENT)) if os.path.isdir(PARENT) else []:",
            "    d = os.path.join(PARENT, name)",
            "    if not name.startswith(stem) or not os.path.isdir(d):",
            "        continue",
            "    cwd = recorded(d)",
            "    if cwd is None and name == own:",
            "        cwd = PATH",
            "    if cwd is not None and under(cwd, PATH):",
            "        say(d)",
        ]
        .map(String::from),
    );
    lines.join("\n")
}

/// listing.ts:127. `join(scratch, home, store)`, node's posix join: one `/` between parts, `.` and `..` resolved,
/// repeated slashes collapsed, a trailing slash kept.
pub fn store_copy(scratch: &str, home: &str, store: &str) -> String {
    join_posix(&[scratch, home, store])
}

/// listing.ts:137. The step that writes a store's rows for the project into its copy, names it and runs the module's
/// steps, each indented eight spaces inside its `try:`.
pub fn filter_step(scratch: &str, home: &str, shared: &SharedStore, steps: &[String]) -> String {
    let mut lines: Vec<String> = vec![
        format!("STORE = {}", py_data(&json!(join_posix(&[home, &shared.store])))),
        format!("COPY = {}", py_data(&json!(store_copy(scratch, home, &shared.store)))),
        "os.makedirs(os.path.dirname(COPY), exist_ok=True)".into(),
        "if os.path.exists(COPY):".into(),
        "    os.remove(COPY)".into(),
        "FILTERED = 0".into(),
        "if os.path.exists(STORE):".into(),
        "    SAID = len(OUT)".into(),
        "    try:".into(),
    ];
    lines.extend(shared.filter.iter().flat_map(|s| indent_lines(s, 8)));
    lines.push("        if FILTERED:".into());
    lines.push("            say(COPY)".into());
    lines.extend(steps.iter().flat_map(|s| indent_lines(s, 8)));
    lines.push("    except Exception as e:".into());
    lines.push("        unread(STORE, COPY, e, SAID)".into());
    lines.join("\n")
}

fn indent_lines(step: &str, spaces: usize) -> Vec<String> {
    let pad = " ".repeat(spaces);
    step.split('\n').map(|line| format!("{pad}{line}")).collect()
}
