// SPDX-License-Identifier: AGPL-3.0-only
//! The TS host's answers to every tape, frozen: one JSON file a tape under `parity/goldens` (or the held-back folder's
//! `goldens`), the hash of the TS tree they were recorded from in that folder's `KEY`, and the host's op list and tool
//! list as they were then in `ops.json` and `tools.json`. A check reads these and never starts the TS host.

use serde_json::Value;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

pub const GOLDENS_DIR: &str = "parity/goldens";
pub const KEY_FILE: &str = "KEY";
pub const OPS_FILE: &str = "parity/goldens/ops.json";
pub const TOOLS_FILE: &str = "parity/goldens/tools.json";

/// What the TS host is: every file under packages that is not a test or a build, tracked or not yet.
const TS_TREE: &[&str] =
    &["packages", ":(exclude,glob)packages/*/test/**", ":(exclude,glob)packages/**/*.test.ts", ":(exclude,glob)packages/wspx/**"];

/// The goldens folder of the repository.
pub fn dir(root: &Path) -> PathBuf {
    root.join(GOLDENS_DIR)
}

pub fn path_of(dir: &Path, tape: &str) -> PathBuf {
    dir.join(format!("{tape}.json"))
}

pub fn read(dir: &Path, tape: &str) -> Result<Option<Value>, String> {
    let path = path_of(dir, tape);
    match fs::read_to_string(&path) {
        Ok(text) => serde_json::from_str(&text).map(Some).map_err(|e| format!("{}: {e}", path.display())),
        Err(_) => Ok(None),
    }
}

pub fn write(dir: &Path, tape: &str, golden: &Value) -> Result<(), String> {
    let path = path_of(dir, tape);
    fs::create_dir_all(path.parent().expect("a golden has a folder")).map_err(|e| e.to_string())?;
    let text = serde_json::to_string_pretty(golden).map_err(|e| e.to_string())? + "\n";
    fs::write(&path, text).map_err(|e| format!("{}: {e}", path.display()))
}

/// Every golden file in a goldens folder, as the tape id it answers.
pub fn recorded(dir: &Path) -> Vec<String> {
    let mut out = Vec::new();
    collect(dir, dir, &mut out);
    out.sort();
    out
}

fn collect(top: &Path, dir: &Path, out: &mut Vec<String>) {
    let Ok(entries) = fs::read_dir(dir) else { return };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect(top, &path, out);
        } else if path.extension().is_some_and(|e| e == "json") && path != top.join("ops.json") && path != top.join("tools.json") {
            out.push(path.strip_prefix(top).expect("under goldens").with_extension("").to_string_lossy().replace('\\', "/"));
        }
    }
}

pub fn read_key(dir: &Path) -> Option<String> {
    fs::read_to_string(dir.join(KEY_FILE)).ok().map(|s| s.trim().to_string())
}

pub fn write_key(dir: &Path, key: &str) -> Result<(), String> {
    fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    fs::write(dir.join(KEY_FILE), format!("{key}\n")).map_err(|e| e.to_string())
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
    write_list(&root.join(OPS_FILE), ops)
}

/// The tool server's tools the goldens were recorded against, each one a verb of the command line as well; none when
/// no list was recorded.
pub fn read_tools(root: &Path) -> Result<Vec<String>, String> {
    match fs::read_to_string(root.join(TOOLS_FILE)) {
        Ok(text) => serde_json::from_str(&text).map_err(|e| format!("{TOOLS_FILE}: {e}")),
        Err(_) => Ok(Vec::new()),
    }
}

pub fn write_tools(root: &Path, tools: &[String]) -> Result<(), String> {
    write_list(&root.join(TOOLS_FILE), tools)
}

fn write_list(path: &Path, list: &[String]) -> Result<(), String> {
    fs::create_dir_all(path.parent().expect("folder")).map_err(|e| e.to_string())?;
    fs::write(path, serde_json::to_string_pretty(list).map_err(|e| e.to_string())? + "\n").map_err(|e| e.to_string())
}
