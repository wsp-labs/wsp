// SPDX-License-Identifier: AGPL-3.0-only
//! The tapes held back from whoever implements a unit: every shadow-traffic tape, and about one in five of the
//! tapes of each op family, the cheapest by a seeded hash of their ids. `cargo xtask unit` never runs them; the
//! reviewer and `cargo xtask parity` always do, so a unit fitted to the tapes it was shown fails on the ones it was not.

use crate::check::family;
use crate::tape::{Door, Tape};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;

/// The seed the held-back set is drawn with; a new seed draws a new set.
pub const SEED_FILE: &str = "parity/held-seed";
/// The folder the day of traffic off a real host is kept in, which no implementer sees any of.
pub const SHADOW_DIR: &str = "shadow/";
/// One tape in this many of each family is held back.
pub const ONE_IN: usize = 5;

pub fn seed(root: &Path) -> Result<u64, String> {
    let text = fs::read_to_string(root.join(SEED_FILE)).map_err(|_| format!("{SEED_FILE} is missing"))?;
    text.trim().parse().map_err(|_| format!("{SEED_FILE}: {} is not a number", text.trim()))
}

/// The ids of the held-back tapes among these.
pub fn held(tapes: &[Tape], seed: u64) -> BTreeSet<String> {
    let mut out: BTreeSet<String> = tapes.iter().filter(|t| t.id.starts_with(SHADOW_DIR)).map(|t| t.id.clone()).collect();
    let mut groups: BTreeMap<String, Vec<&Tape>> = BTreeMap::new();
    for tape in tapes.iter().filter(|t| !t.id.starts_with(SHADOW_DIR)) {
        // Every websocket tape sends auth first, so auth names no family of its own here.
        let families: BTreeSet<String> = match tape.door {
            Door::Ws => tape.ops().iter().map(|op| family(op).to_string()).filter(|f| f != "auth").collect(),
            door => [format!("({} door)", door.word())].into(),
        };
        for f in families {
            groups.entry(f).or_default().push(tape);
        }
    }
    for members in groups.values_mut() {
        members.sort_by_key(|t| (rank(seed, &t.id), t.id.clone()));
        let quota = members.len().div_ceil(ONE_IN);
        let already = members.iter().filter(|t| out.contains(&t.id)).count();
        let mut wanted = quota.saturating_sub(already);
        for t in members.iter() {
            if wanted == 0 {
                break;
            }
            if out.insert(t.id.clone()) {
                wanted -= 1;
            }
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
