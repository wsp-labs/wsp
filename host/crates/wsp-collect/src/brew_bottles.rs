// SPDX-License-Identifier: AGPL-3.0-only
//! Homebrew publishes one JSON document for homebrew/core with every formula's bottles. The collector ships a dated
//! snapshot of the names, split by whether any bottle is built for Linux, so the tools rung can grey out Mac-only
//! formulae without a network call. Port of packages/collect/src/brew-bottles.ts at a3b870a6e; the spec is
//! spec/brew_bottles.md.
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::manifest::Linux;

pub use crate::data::linux_bottles::LINUX_BOTTLES;

/// brew-bottles.ts:9. Keys in this order, which is also the embedded file's.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BottleSnapshot {
    /// YYYY-MM-DD the formula list was fetched.
    pub fetched: String,
    pub source: String,
    /// Core formulae with an `all`, `x86_64_linux` or `arm64_linux` bottle.
    pub linux: Vec<String>,
    /// Core formulae with bottles for macOS only, or none at all.
    pub no_linux: Vec<String>,
}

/// brew-bottles.ts:19, in this order.
pub const LINUX_BOTTLE_TAGS: [&str; 3] = ["all", "x86_64_linux", "arm64_linux"];

/// The two name lists `classify_formulae` answers (TS `Pick<BottleSnapshot, "linux" | "noLinux">`).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Classified {
    pub linux: Vec<String>,
    pub no_linux: Vec<String>,
}

/// Splits formulae.brew.sh's formula.json into the two name lists, each sorted by UTF-16 code units.
pub fn classify_formulae(data: &Value) -> Classified {
    let mut out = Classified::default();
    let Value::Array(formulae) = data else { return out };
    for f in formulae {
        let (Some(name), Some(f)) = (f.get("name").and_then(Value::as_str), f.as_object()) else { continue };
        let files = f.get("bottle").and_then(|b| b.get("stable")).and_then(|s| s.get("files")).and_then(Value::as_object);
        let linux = files.is_some_and(|files| LINUX_BOTTLE_TAGS.iter().any(|tag| files.contains_key(*tag)));
        (if linux { &mut out.linux } else { &mut out.no_linux }).push(name.to_string());
    }
    out.linux.sort_by(|a, b| wsp_words::js::utf16::cmp(a, b));
    out.no_linux.sort_by(|a, b| wsp_words::js::utf16::cmp(a, b));
    out
}

/// The text of data/linux-bottles.json for a snapshot: `serde_json::to_string_pretty` (two-space indent, keys
/// fetched, source, linux, noLinux) and one trailing newline. The TS renders its generated .ts file here
/// (brew-bottles.ts:45); the Rust crate's generated file is that JSON, which data/linux_bottles.py writes the same way.
pub fn render_snapshot(s: &BottleSnapshot) -> String {
    let mut text = serde_json::to_string_pretty(s).expect("a snapshot of strings serializes");
    text.push('\n');
    text
}

/// Yes for a name in `linux`, no for one in `no_linux`, unknown for any other (a tap, or one added since the snapshot).
/// The TS keeps a Set per snapshot object; a lookup that is linear in the lists is a behaviour difference only in time.
pub fn linux_support(formula: &str, snapshot: &BottleSnapshot) -> Linux {
    if snapshot.linux.iter().any(|n| n == formula) {
        Linux::Yes
    } else if snapshot.no_linux.iter().any(|n| n == formula) {
        Linux::No
    } else {
        Linux::Unknown
    }
}
