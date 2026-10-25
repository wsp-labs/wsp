// SPDX-License-Identifier: AGPL-3.0-only
//! The held-back tapes are the checks an implementer never sees (#2440: they "stay out of the packet; the reviewer
//! runs them"), so none of them, none of their goldens and not the seed that draws them is in the tree a unit's
//! implementer checks out, and so in no commit main will ever hold.

use std::collections::BTreeSet;
use std::path::Path;
use std::process::Command;
use xtask::held::{held, SEED_FILE};
use xtask::tape;

#[test]
fn no_held_back_tape_no_golden_of_one_and_not_the_seed_is_in_the_tree() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
    let out = Command::new("git").args(["ls-files", "parity"]).current_dir(&root).output().expect("git");
    let tracked: BTreeSet<String> = String::from_utf8_lossy(&out.stdout).lines().map(str::to_string).collect();
    assert!(!tracked.contains(SEED_FILE), "{SEED_FILE} is in the tree, so cargo xtask parity held prints the held-back set there");
    let readable: Vec<String> = held(&tape::load(&root).expect("tapes"))
        .into_iter()
        .filter(|id| tracked.contains(&format!("parity/tapes/{id}.tape")) || tracked.contains(&format!("parity/goldens/{id}.json")))
        .collect();
    assert!(readable.is_empty(), "{} held-back tapes are in the tree with their goldens, as {}", readable.len(), readable[0]);
}

/// The frames a tape sends, its numbered redactions read alike, so a tape collected again reads as the one before it.
fn sends(text: &str) -> Option<String> {
    let numbered = regex::Regex::new(r"<redacted-\d+>").unwrap();
    let ins: Vec<serde_json::Value> = text
        .lines()
        .filter_map(|l| serde_json::from_str::<serde_json::Value>(l).ok())
        .filter_map(|l| l.get("in").cloned())
        .collect();
    (ins.len() > 1).then(|| numbered.replace_all(&serde_json::Value::from(ins).to_string(), "<redacted>").into_owned())
}

#[test]
fn no_commit_of_the_branch_holds_what_a_held_back_tape_sends_under_any_name() {
    // The held-back folder is the reviewer's alone, so this runs where WSP_PARITY_HELD names it.
    let Some(held) = std::env::var_os("WSP_PARITY_HELD").map(std::path::PathBuf::from) else {
        eprintln!("skipped: WSP_PARITY_HELD names no held-back folder");
        return;
    };
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
    let git = |args: &[&str]| String::from_utf8(Command::new("git").args(args).current_dir(&root).output().expect("git").stdout).unwrap();
    let held_sends: BTreeSet<String> = tape::load_held(&held)
        .expect("held tapes")
        .iter()
        .filter_map(|t| std::fs::read_to_string(held.join("tapes").join(format!("{}.tape", t.id))).ok())
        .filter_map(|text| sends(&text))
        .collect();
    let mut seen = BTreeSet::new();
    let mut readable = Vec::new();
    for commit in git(&["log", "--format=%H", "HEAD", "--", "parity/tapes"]).lines() {
        for line in git(&["ls-tree", "-r", commit, "--", "parity/tapes"]).lines() {
            let Some((meta, path)) = line.split_once('\t') else { continue };
            let blob = meta.split_whitespace().nth(2).unwrap_or_default().to_string();
            if !path.ends_with(".tape") || !seen.insert(blob.clone()) {
                continue;
            }
            if sends(&git(&["cat-file", "-p", &blob])).is_some_and(|s| held_sends.contains(&s)) {
                readable.push(format!("{} {path}", &commit[..9]));
            }
        }
    }
    assert!(readable.is_empty(), "{} tapes in the branch's history send what a held-back tape sends, as {}", readable.len(), readable[0]);
}
