// SPDX-License-Identifier: AGPL-3.0-only
//! `cargo xtask unit <name>`: one unit of the port held to what its packet fixed. It refuses a changed test file, a
//! changed tape, golden or spec, a path changed outside the unit, and a `todo!()` or `unimplemented!()` left in it;
//! then runs the unit's tests and the tapes that send its ops, less the held-back ones.
//!
//! `parity/units.tsv` names each unit, one a line: `unit<TAB>manifest<TAB>package<TAB>src<TAB>tests<TAB>filter<TAB>ops`.
//! `src` is the unit's folder or file, `tests` its test files or folders split by commas, `filter` the name the test
//! run is narrowed to or `-`, and `ops` the host ops it answers, split by commas, `*` matching any run of characters.

use crate::accepted::glob;
use crate::tape::{Door, Tape};
use std::collections::BTreeSet;
use std::fs;
use std::path::Path;
use std::process::Command;

pub const UNITS_FILE: &str = "parity/units.tsv";
/// What a unit's implementer never changes: the tapes, their answers, the spec and the harness's own lists.
const FIXED: &[&str] = &["parity/tapes/", "parity/goldens/", "parity/accepted.tsv", UNITS_FILE, "docs/rust-host-spec/"];

#[derive(Clone, Debug)]
pub struct Unit {
    pub name: String,
    pub manifest: String,
    pub package: String,
    pub src: String,
    pub tests: Vec<String>,
    pub filter: Option<String>,
    pub ops: Vec<String>,
}

pub fn read_units(root: &Path) -> Result<Vec<Unit>, String> {
    let Ok(text) = fs::read_to_string(root.join(UNITS_FILE)) else { return Ok(Vec::new()) };
    let mut units = Vec::new();
    for (n, line) in text.lines().enumerate() {
        if line.trim().is_empty() || line.starts_with('#') {
            continue;
        }
        let cols: Vec<&str> = line.split('\t').collect();
        let [name, manifest, package, src, tests, filter, ops] = cols.as_slice() else {
            return Err(format!(
                "{UNITS_FILE}:{}: a unit is seven columns split by tabs: unit, manifest, package, src, tests, filter, ops",
                n + 1
            ));
        };
        let list = |s: &str| s.split(',').map(str::trim).filter(|p| !p.is_empty()).map(str::to_string).collect::<Vec<_>>();
        units.push(Unit {
            name: name.to_string(),
            manifest: manifest.to_string(),
            package: package.to_string(),
            src: src.trim_end_matches('/').to_string(),
            tests: list(tests),
            filter: (*filter != "-").then(|| filter.to_string()),
            ops: list(ops),
        });
    }
    Ok(units)
}

/// Every path the work changed since `base`: committed, staged, in the working tree, and new files git does not ignore.
pub fn changed(root: &Path, base: &str) -> Result<Vec<String>, String> {
    let mut paths: BTreeSet<String> = git(root, &["diff", "--name-only", base, "--"])?.lines().map(str::to_string).collect();
    paths.extend(git(root, &["ls-files", "--others", "--exclude-standard"])?.lines().map(str::to_string));
    Ok(paths.into_iter().filter(|p| !p.is_empty()).collect())
}

/// What the unit's work did that its packet does not allow, one sentence each; none is a pass.
pub fn refusals(root: &Path, unit: &Unit, changed: &[String]) -> Vec<String> {
    let under = |path: &str, top: &str| path == top || path.starts_with(&format!("{}/", top.trim_end_matches('/')));
    let mut out = Vec::new();
    for path in changed {
        if unit.tests.iter().any(|t| under(path, t)) || FIXED.iter().any(|f| path.starts_with(f) || path == f.trim_end_matches('/')) {
            out.push(format!("changes a test or a tape, which the packet fixed: {path}"));
        } else if !under(path, &unit.src) {
            out.push(format!("changes a path outside the unit ({}): {path}", unit.src));
        }
    }
    let mut files = Vec::new();
    rust_files(&root.join(&unit.src), &mut files);
    for file in files {
        let Ok(text) = fs::read_to_string(&file) else { continue };
        let rel = file.strip_prefix(root).unwrap_or(&file).display().to_string();
        for (n, line) in text.lines().enumerate() {
            for stub in ["todo!(", "unimplemented!("] {
                if line.contains(stub) {
                    out.push(format!("{rel}:{}: {stub}) is left in the unit", n + 1));
                }
            }
        }
    }
    out
}

fn rust_files(path: &Path, out: &mut Vec<std::path::PathBuf>) {
    if path.is_file() {
        if path.extension().is_some_and(|e| e == "rs") {
            out.push(path.to_path_buf());
        }
        return;
    }
    let Ok(entries) = fs::read_dir(path) else { return };
    let mut entries: Vec<_> = entries.flatten().map(|e| e.path()).collect();
    entries.sort();
    for p in entries {
        rust_files(&p, out);
    }
}

/// The tapes the unit is run against: those that send any of its ops, less the held-back ones.
pub fn visible_tapes(unit: &Unit, tapes: &[Tape], held: &BTreeSet<String>) -> BTreeSet<String> {
    tapes
        .iter()
        .filter(|t| t.door == Door::Ws && !held.contains(&t.id))
        .filter(|t| t.ops().iter().any(|op| op != "auth" && unit.ops.iter().any(|g| glob(g, op))))
        .map(|t| t.id.clone())
        .collect()
}

/// The unit's own tests, through cargo, with the output going straight to the terminal. They run inside the sandbox,
/// which has no network and a read-only disk but the unit's build folder: so offline, and with no compiler wrapper,
/// whose cache server is out of reach there.
pub fn run_tests(root: &Path, unit: &Unit) -> Result<bool, String> {
    let mut cmd = Command::new(std::env::var("CARGO").unwrap_or_else(|_| "cargo".into()));
    cmd.current_dir(root).env("RUSTC_WRAPPER", "").args(test_args(unit));
    cmd.status().map(|s| s.success()).map_err(|e| format!("cargo test: {e}"))
}

/// What `cargo test` is handed for a unit. Test files under the crate's `tests/` run as their own targets and no other,
/// since cargo stops at the first red target and another packet's file stays red until that packet lands; a folder
/// runs the package's tests.
pub fn test_args(unit: &Unit) -> Vec<String> {
    let mut args: Vec<String> =
        ["test", "--offline", "--manifest-path", &unit.manifest, "--package", &unit.package].map(String::from).to_vec();
    let tests_dir = Path::new(&unit.manifest).parent().unwrap_or(Path::new("")).join("tests");
    let stems: Option<Vec<String>> = unit
        .tests
        .iter()
        .map(|t| {
            let t = Path::new(t);
            (t.parent() == Some(tests_dir.as_path()) && t.extension().is_some_and(|e| e == "rs"))
                .then(|| t.file_stem().map(|s| s.to_string_lossy().into_owned()))
                .flatten()
        })
        .collect();
    for stem in stems.unwrap_or_default() {
        args.extend(["--test".to_string(), stem]);
    }
    if let Some(filter) = &unit.filter {
        args.extend(["--".to_string(), filter.clone()]);
    }
    args
}

/// The folder cargo builds the unit in, the one the sandbox leaves writable for its tests.
pub fn target_dir(root: &Path, unit: &Unit) -> Result<std::path::PathBuf, String> {
    let out = Command::new(std::env::var("CARGO").unwrap_or_else(|_| "cargo".into()))
        .current_dir(root)
        .args(["metadata", "--no-deps", "--format-version", "1", "--manifest-path", &unit.manifest])
        .output()
        .map_err(|e| format!("cargo metadata: {e}"))?;
    let meta: serde_json::Value = serde_json::from_slice(&out.stdout).map_err(|e| format!("cargo metadata: {e}"))?;
    let dir = meta["target_directory"].as_str().ok_or("cargo metadata named no target folder")?;
    fs::create_dir_all(dir).map_err(|e| format!("{dir}: {e}"))?;
    Ok(dir.into())
}

pub fn merge_base(root: &Path) -> Result<String, String> {
    Ok(git(root, &["merge-base", "origin/main", "HEAD"])?.trim().to_string())
}

fn git(root: &Path, args: &[&str]) -> Result<String, String> {
    let out = Command::new("git").current_dir(root).args(args).output().map_err(|e| format!("git: {e}"))?;
    if !out.status.success() {
        return Err(format!("git {}: {}", args.join(" "), String::from_utf8_lossy(&out.stderr).trim()));
    }
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}
