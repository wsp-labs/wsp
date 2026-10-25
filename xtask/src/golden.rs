// SPDX-License-Identifier: AGPL-3.0-only
//! The TS host's answers to every tape, frozen: one JSON file a tape under `parity/goldens`, the hash of the TS tree
//! they were recorded from in `KEY`, and the host's op list as it was then in `ops.json`. A check reads these and
//! never starts the TS host.

use serde_json::Value;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

pub const GOLDENS_DIR: &str = "parity/goldens";
pub const KEY_FILE: &str = "parity/goldens/KEY";
pub const OPS_FILE: &str = "parity/goldens/ops.json";

/// What the TS host is: every file under packages that is not a test or a build, tracked or not yet.
const TS_TREE: &[&str] =
    &["packages", ":(exclude,glob)packages/*/test/**", ":(exclude,glob)packages/**/*.test.ts", ":(exclude,glob)packages/wspx/**"];

pub fn path_of(root: &Path, tape: &str) -> PathBuf {
    root.join(GOLDENS_DIR).join(format!("{tape}.json"))
}

pub fn read(root: &Path, tape: &str) -> Result<Option<Value>, String> {
    let path = path_of(root, tape);
    match fs::read_to_string(&path) {
        Ok(text) => serde_json::from_str(&text).map(Some).map_err(|e| format!("{}: {e}", path.display())),
        Err(_) => Ok(None),
    }
}

pub fn write(root: &Path, tape: &str, golden: &Value) -> Result<(), String> {
    let path = path_of(root, tape);
    fs::create_dir_all(path.parent().expect("a golden has a folder")).map_err(|e| e.to_string())?;
    let text = serde_json::to_string_pretty(golden).map_err(|e| e.to_string())? + "\n";
    fs::write(&path, text).map_err(|e| format!("{}: {e}", path.display()))
}

/// Every golden file on disk, as the tape id it answers.
pub fn recorded(root: &Path) -> Vec<String> {
    let dir = root.join(GOLDENS_DIR);
    let mut out = Vec::new();
    collect(&dir, &dir, &mut out);
    out.sort();
    out
}

fn collect(top: &Path, dir: &Path, out: &mut Vec<String>) {
    let Ok(entries) = fs::read_dir(dir) else { return };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect(top, &path, out);
        } else if path.extension().is_some_and(|e| e == "json") && path != top.join("ops.json") {
            out.push(path.strip_prefix(top).expect("under goldens").with_extension("").to_string_lossy().replace('\\', "/"));
        }
    }
}

pub fn read_key(root: &Path) -> Option<String> {
    fs::read_to_string(root.join(KEY_FILE)).ok().map(|s| s.trim().to_string())
}

/// The hash of the TS tree as it stands in the working copy: git's hash of each file's bytes, then of the list of
/// paths and hashes, so an edit not yet committed moves it as a commit would.
pub fn tree_key(root: &Path) -> Result<String, String> {
    let listed = git(root, &[&["ls-files", "--cached", "--others", "--exclude-standard", "-z", "--"][..], TS_TREE].concat(), None)?;
    let mut paths: Vec<&str> = listed.split('\0').filter(|p| !p.is_empty() && root.join(p).is_file()).collect();
    paths.sort();
    paths.dedup();
    let hashes = git(root, &["hash-object", "--stdin-paths"], Some(&paths.join("\n")))?;
    let list: String = paths.iter().zip(hashes.lines()).map(|(p, h)| format!("{h} {p}\n")).collect();
    let key = git(root, &["hash-object", "--stdin"], Some(&list))?;
    Ok(key.trim().to_string())
}

fn git(root: &Path, args: &[&str], stdin: Option<&str>) -> Result<String, String> {
    let mut child = Command::new("git")
        .current_dir(root)
        .args(args)
        .stdin(if stdin.is_some() { Stdio::piped() } else { Stdio::null() })
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("git: {e}"))?;
    if let Some(text) = stdin {
        let mut pipe = child.stdin.take().expect("piped");
        let text = text.to_string();
        std::thread::spawn(move || {
            let _ = pipe.write_all(text.as_bytes());
        });
    }
    let out = child.wait_with_output().map_err(|e| format!("git: {e}"))?;
    if !out.status.success() {
        return Err(format!("git {}: {}", args.join(" "), String::from_utf8_lossy(&out.stderr).trim()));
    }
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}

/// The host ops the goldens were recorded against, which coverage is counted over.
pub fn read_ops(root: &Path) -> Result<Vec<String>, String> {
    let path = root.join(OPS_FILE);
    let text = fs::read_to_string(&path).map_err(|_| format!("{OPS_FILE} is missing: run cargo xtask parity record"))?;
    serde_json::from_str(&text).map_err(|e| format!("{OPS_FILE}: {e}"))
}

pub fn write_ops(root: &Path, ops: &[String]) -> Result<(), String> {
    let path = root.join(OPS_FILE);
    fs::create_dir_all(path.parent().expect("folder")).map_err(|e| e.to_string())?;
    fs::write(&path, serde_json::to_string_pretty(ops).map_err(|e| e.to_string())? + "\n").map_err(|e| e.to_string())
}
