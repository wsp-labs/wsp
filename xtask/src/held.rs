// SPDX-License-Identifier: AGPL-3.0-only
//! The tapes held back from whoever implements a unit live outside the repository, in a folder only the reviewer's
//! checkout has, given as `--held <dir>`: `<dir>/tapes` beside the recorded ones' layout, their goldens in
//! `<dir>/goldens`, and the seed that drew them in `<dir>/seed`. `cargo xtask unit` never sees them; the reviewer's
//! `cargo xtask parity --held <dir>` runs them, so a unit fitted to the tapes it was shown fails on the ones it was
//! not. `draw` picks the tapes to move there: every shadow-traffic tape, and about one in five of each op family's,
//! the cheapest by a seeded hash of their ids; never a spec example, a hand-written tape, or a family's last tape left
//! in the repository.

use crate::check::{family, verb_of};
use crate::tape::{Door, Tape};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;

/// The seed the held-back set was drawn with, inside the held-back folder; a new seed draws a new set.
pub const SEED_FILE: &str = "seed";
/// The folder the day of traffic off a real host is kept in, which no implementer sees any of.
pub const SHADOW_DIR: &str = "shadow/";
/// One tape in this many of each family is held back.
pub const ONE_IN: usize = 5;
/// Tapes every implementer is shown: the spec's own examples, and the ones written by hand to pin one rule down, as
/// key order is.
pub const ALWAYS_SHOWN: &[&str] = &["spec/", "hand/"];

/// The seed in a held-back folder.
pub fn seed(dir: &Path) -> Result<u64, String> {
    let path = dir.join(SEED_FILE);
    let text = fs::read_to_string(&path).map_err(|_| format!("{} is missing", path.display()))?;
    text.trim().parse().map_err(|_| format!("{}: {} is not a number", path.display(), text.trim()))
}

/// The ids among these tapes that are held back: those read from the held-back folder, and any shadow tape.
pub fn held(tapes: &[Tape]) -> BTreeSet<String> {
    tapes.iter().filter(|t| t.held || t.id.starts_with(SHADOW_DIR)).map(|t| t.id.clone()).collect()
}

/// The families a tape counts toward: its websocket ops' families less auth, which every websocket tape sends first;
/// for the tool server each tool it calls and for the command line its verb, as `tools` names them, so a draw never
/// takes the last tape a tool has on either door.
fn families(tape: &Tape, tools: &[String]) -> BTreeSet<String> {
    match tape.door {
        Door::Ws => tape.ops().iter().map(|op| family(op).to_string()).filter(|f| f != "auth").collect(),
        Door::Mcp => {
            let called: BTreeSet<String> =
                tape.ins.iter().filter_map(|f| f.pointer("/params/name").and_then(Value::as_str)).map(|t| format!("(mcp) {t}")).collect();
            if called.is_empty() {
                ["(mcp door)".to_string()].into()
            } else {
                called
            }
        }
        Door::Cli => tape
            .ins
            .iter()
            .map(|f| format!("(cli) {}", verb_of(f.get("argv").and_then(Value::as_array).map(Vec::as_slice).unwrap_or_default(), tools)))
            .collect(),
    }
}

/// The ids the seed holds back among these tapes, those already held counted first: every shadow tape, then each
/// family's quota of one in five, never taking the last tape any family has left to show.
pub fn draw(tapes: &[Tape], seed: u64, tools: &[String]) -> BTreeSet<String> {
    let mut out = held(tapes);
    let drawable: Vec<&Tape> =
        tapes.iter().filter(|t| !t.id.starts_with(SHADOW_DIR) && !ALWAYS_SHOWN.iter().any(|p| t.id.starts_with(p))).collect();
    let mut groups: BTreeMap<String, Vec<&Tape>> = BTreeMap::new();
    for tape in &drawable {
        for f in families(tape, tools) {
            groups.entry(f).or_default().push(tape);
        }
    }
    let mut shown: BTreeMap<String, usize> = BTreeMap::new();
    for tape in tapes.iter().filter(|t| !out.contains(&t.id)) {
        for f in families(tape, tools) {
            *shown.entry(f).or_default() += 1;
        }
    }
    for members in groups.values_mut() {
        members.sort_by_key(|t| (rank(seed, &t.id), t.id.clone()));
        let quota = members.len().div_ceil(ONE_IN);
        let mut wanted = quota.saturating_sub(members.iter().filter(|t| out.contains(&t.id)).count());
        for t in members.iter() {
            if wanted == 0 {
                break;
            }
            let fs = families(t, tools);
            if out.contains(&t.id) || fs.iter().any(|f| shown.get(f).copied().unwrap_or(0) <= 1) {
                continue;
            }
            out.insert(t.id.clone());
            for f in fs {
                *shown.get_mut(&f).expect("counted") -= 1;
            }
            wanted -= 1;
        }
    }
    out
}

fn rank(seed: u64, id: &str) -> u64 {
    let mut h = seed ^ 0xcbf2_9ce4_8422_2325;
    for b in id.bytes() {
        h = (h ^ u64::from(b)).wrapping_mul(0x0100_0000_01b3);
    }
    h ^ (h >> 33)
}
