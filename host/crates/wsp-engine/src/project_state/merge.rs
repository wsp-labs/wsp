// SPDX-License-Identifier: AGPL-3.0-only
//! Port of packages/engine/src/project-state/merge.ts. Spec: spec/project_state_scripts.md.
//!
//! The python3 script the machine runs once a project's files have landed, for rows an agent keeps in a store shared
//! with every project.
use serde_json::{json, Map, Value};

use crate::errors::{EngineError, Result};
use crate::project_state::py::{py_data, PY_PREAMBLE};
use crate::project_state::resolver::js_read_kept;

/// merge.ts:11. What the script prints as its last line.
#[derive(Debug, Clone, PartialEq)]
pub enum MergeOutput {
    Merged { merged: f64, kept: f64, note: Option<String> },
    Waiting { waiting: String },
}

/// merge.ts:14. The last line of the stdout with its trailing whitespace trimmed. Empty: `the merge script printed
/// nothing`. Not JSON, or JSON of neither shape: `the merge script ended with: <line>`. A string `waiting` wins over
/// counts; `merged` and `kept` must both be JSON numbers; a `note` that is not a string is dropped.
pub fn parse_merge_output(stdout: &str) -> Result<MergeOutput> {
    let trimmed = wsp_words::js::js_trim_end(stdout);
    let last = trimmed.rsplit('\n').next().unwrap_or("");
    if last.is_empty() {
        return Err(EngineError::Other { message: "the merge script printed nothing".into(), code: None });
    }
    let ended = || EngineError::Other { message: format!("the merge script ended with: {last}"), code: None };
    let parsed = js_read_kept(last).ok_or_else(ended)?.0;
    let o: Map<String, Value> = match parsed {
        Value::Object(o) => o,
        _ => Map::new(),
    };
    if let Some(Value::String(waiting)) = o.get("waiting") {
        return Ok(MergeOutput::Waiting { waiting: waiting.clone() });
    }
    match (count(&o, "merged"), count(&o, "kept")) {
        (Some(merged), Some(kept)) => {
            let note = o.get("note").and_then(Value::as_str).map(str::to_string);
            Ok(MergeOutput::Merged { merged, kept, note })
        }
        _ => Err(ended()),
    }
}

fn count(o: &Map<String, Value>, key: &str) -> Option<f64> {
    o.get(key).and_then(Value::as_f64)
}

/// merge.ts:37. A merge script from its steps, byte for byte as the TS builds it (pinned by the oracle).
pub fn merge_script(from: &str, to: &str, steps: &[String]) -> String {
    let mut lines: Vec<String> = vec![
        PY_PREAMBLE.to_string(),
        "def moved(p):".into(),
        "    return TO + p[len(FROM):] if under(p, FROM) else None".into(),
        "def out(o):".into(),
        "    print(json.dumps(o))".into(),
        "    sys.exit(0)".into(),
        "def fail(why):".into(),
        "    print(why, file=sys.stderr)".into(),
        "    sys.exit(1)".into(),
        "def write(path, text):".into(),
        "    tmp = path + \".wsp-merge\"".into(),
        "    with open(tmp, \"w\", encoding=\"utf-8\", errors=\"surrogateescape\", newline=\"\") as f:".into(),
        "        f.write(text)".into(),
        "    if os.path.exists(path):".into(),
        "        shutil.copymode(path, tmp)".into(),
        "    os.replace(tmp, path)".into(),
        format!("FROM = {}", py_data(&json!(from))),
        format!("TO = {}", py_data(&json!(to))),
        "merged = 0".into(),
        "kept = 0".into(),
        "notes = []".into(),
    ];
    lines.extend(steps.iter().cloned());
    lines.push("out({\"merged\": merged, \"kept\": kept, **({\"note\": \"; \".join(notes)} if notes else {})})".into());
    lines.join("\n")
}

/// merge.ts:68. The step rewriting `cwd` on JSON lines of the files, inside the object `holder` names or the line
/// itself.
pub fn jsonl_cwd_step(files: &[String], holder: Option<&str>) -> String {
    let holder_line = match holder {
        None => "        holder = obj".to_string(),
        Some(key) => format!("        holder = obj.get({})", py_data(&json!(key))),
    };
    let lines: Vec<String> = vec![
        format!("for f in {}:", py_data(&json!(files))),
        "    if not os.path.exists(f):".into(),
        "        continue".into(),
        "    with open(f, encoding=\"utf-8\", errors=\"surrogateescape\", newline=\"\") as h:".into(),
        "        parts = h.read().split(\"\\n\")".into(),
        "    changed = False".into(),
        "    for i, line in enumerate(parts):".into(),
        "        try:".into(),
        "            obj = json.loads(line)".into(),
        "        except ValueError:".into(),
        "            continue".into(),
        "        if not isinstance(obj, dict):".into(),
        "            continue".into(),
        holder_line,
        "        if not isinstance(holder, dict):".into(),
        "            continue".into(),
        "        cwd = holder.get(\"cwd\")".into(),
        "        target = moved(cwd) if isinstance(cwd, str) else None".into(),
        "        if target is None:".into(),
        "            continue".into(),
        "        holder[\"cwd\"] = target".into(),
        "        parts[i] = json.dumps(obj, ensure_ascii=False, separators=(\",\", \":\"))".into(),
        "        changed = True".into(),
        "    if changed:".into(),
        "        write(f, \"\\n\".join(parts))".into(),
    ];
    lines.join("\n")
}
