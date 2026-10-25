// SPDX-License-Identifier: AGPL-3.0-only
//! Port of packages/engine/src/project-state/py.ts. Spec: spec/project_state_scripts.md.
//!
//! The lines every python3 script a module hands the machine opens with.
use serde_json::Value;

/// py.ts:8. `data(<JSON string of the standard base64 of the UTF-8 of JSON.stringify(value)>)`: the JSON is
/// `wsp_catalog::jsonc::js_stringify`'s (JS key order and number spelling), the base64 padded.
pub fn py_data(value: &Value) -> String {
    todo!()
}

/// py.ts:13. The imports, data(), under() and q(), newline-joined, no trailing newline.
pub const PY_PREAMBLE: &str = "import base64, json, os, re, shutil, sqlite3, sys, urllib.parse\ndef data(b):\n    return json.loads(base64.b64decode(b))\ndef under(p, root):\n    return p == root or p.startswith(root + \"/\")\ndef q(name):\n    return '\"' + name.replace('\"', '\"\"') + '\"'";
