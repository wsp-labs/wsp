// SPDX-License-Identifier: AGPL-3.0-only
//! Port of packages/engine/src/project-state/py.ts. Spec: spec/project_state_scripts.md.
//!
//! The lines every python3 script a module hands the machine opens with.
use base64::Engine;
use serde_json::Value;

use crate::project_state::resolver::js_write;

/// py.ts:8. `data(<JSON string of the standard base64 of the UTF-8 of JSON.stringify(value)>)`, the base64 padded. The
/// JSON is `js_write`'s: number spelling as JS writes it, and every object's canonical array-index keys (`0` to
/// `4294967294`, no sign, no leading zero) first in ascending order, the rest in place (spec/project_state_scripts.md M10).
pub fn py_data(value: &Value) -> String {
    format!("data(\"{}\")", base64::engine::general_purpose::STANDARD.encode(js_write(value, None)))
}

/// py.ts:13. The imports, data(), under() and q(), newline-joined, no trailing newline.
pub const PY_PREAMBLE: &str = "import base64, json, os, re, shutil, sqlite3, sys, urllib.parse\ndef data(b):\n    return json.loads(base64.b64decode(b))\ndef under(p, root):\n    return p == root or p.startswith(root + \"/\")\ndef q(name):\n    return '\"' + name.replace('\"', '\"\"') + '\"'";
