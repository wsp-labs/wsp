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
    let seed = xtask::held::seed(&root).unwrap_or(2443);
    let readable: Vec<String> = held(&tape::load(&root).expect("tapes"), seed)
        .into_iter()
        .filter(|id| tracked.contains(&format!("parity/tapes/{id}.tape")) || tracked.contains(&format!("parity/goldens/{id}.json")))
        .collect();
    assert!(readable.is_empty(), "{} held-back tapes are in the tree with their goldens, as {}", readable.len(), readable[0]);
}
