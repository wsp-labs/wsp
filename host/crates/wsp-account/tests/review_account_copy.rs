// SPDX-License-Identifier: AGPL-3.0-only
//! Review of the served and copy channels over generated scripts. `oracle/zz-review-account-copy-fuzz.oracle.ts` ran the TS
//! `channelsArea` on the fake context of oracle/channels_copy.oracle.ts over 300 seeded worlds and scripts (a machine of a
//! provider with a ready broker or not, a folder, a copy on a joined computer, a blocked computer, a local copy; creates with
//! odd thread, cwd and env values, lists, kills, events, closes, a daemon that forgets, a delete, a restart, a held reply,
//! a terminal write that never answers) and wrote each world and script beside the trace the TS made of it, as
//! tests/fixtures/oracle/zz_review_account_copy_fuzz.json. The context is tests/channels_copy.rs's.
#![allow(dead_code, unused_imports)]
mod common;
include!("channels_copy/world.rs");

fn named(name: &str) -> (Value, Value, Value) {
    let path = format!("{}/tests/fixtures/oracle/zz_review_account_copy_named.json", env!("CARGO_MANIFEST_DIR"));
    let all: Value = serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
    let s = all[name].clone();
    let (steps, fx) = run_one(s["world"].clone(), s["script"].as_array().unwrap().clone(), Mode::Copy);
    assert_eq!(steps, s["trace"]["steps"], "{name}: the steps' answers");
    (fx, s["trace"]["fx"].clone(), s["world"].clone())
}

// ---------------------------------------------------------------------------------------------------------------
// R-ACC-D1. channels.ts:760 `await Promise.all(gone.map(revokeShellToken))`: every token of the shells that ended is taken
// away at once, so one door that is slow, or never answers, holds back none of the others. copy.rs:523 `shells_ended`
// revokes one after another and waits on each. Node (zz-review-account-copy-fuzz.oracle.ts "two shells end and the door
// refuses both"): both revokes are asked before either refusal is warned. Written as the trace the host leaves.

#[test]
fn every_token_of_the_shells_that_ended_is_asked_for_before_any_answer_is_waited_on() {
    let (got, want, _) = named("two shells end and the door refuses both");
    assert_eq!(got.to_string(), want.to_string());
}

// ---------------------------------------------------------------------------------------------------------------
// R-ACC-D2. channels.ts:814-815 `typeof asked["env"] === "object" && asked["env"] !== null ? asked["env"] : {}` and
// `{ ...threadEnv, ...env, ...minted.env }`: an `env` that is a list is an object to JavaScript and spreads as its indices
// ("0", "1"), as box_panels.rs:579 does for the same frame on a folder. copy.rs:246 takes only a JSON object, so the same
// pty.create on a brokered machine reaches the daemon without them. Node (same oracle, "a page names its env as a list"):
// env {"0":"z","1":"y", then the thread's and the token's}.

#[test]
fn a_list_for_env_spreads_as_its_indices_as_it_does_in_the_ts() {
    let (got, want, _) = named("a page names its env as a list");
    assert_eq!(got.to_string(), want.to_string());
}

// ---------------------------------------------------------------------------------------------------------------
// Where a child starts, over 400 generated leads (a status read with any head, upstream, ahead count, changes, default
// branch and unknown counts, a push that answers or is refused in each of its ways, a base on the record or the project,
// a worktree) and 400 children put on a branch (the answer taken or refused, a worktree or not, a delete or a persist that
// fails): oracle/zz-review-account-copy-fuzz.oracle.ts "probe: lead fuzz".
