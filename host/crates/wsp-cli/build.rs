// SPDX-License-Identifier: AGPL-3.0-only
//! The release this build carries: the `version` of `packages/host/package.json`, the file the TS host reads too, so
//! one bump moves both hosts.

use std::{env, fs, path::Path};

fn main() {
    let path =
        Path::new(&env::var("CARGO_MANIFEST_DIR").expect("cargo sets CARGO_MANIFEST_DIR")).join("../../../packages/host/package.json");
    println!("cargo::rerun-if-changed={}", path.display());
    let text = fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    let parsed: serde_json::Value = serde_json::from_str(&text).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    let version = parsed["version"].as_str().unwrap_or_else(|| panic!("{} holds no string version", path.display()));
    println!("cargo::rustc-env=WSP_RELEASE={version}");
}
