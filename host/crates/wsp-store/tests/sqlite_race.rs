// SPDX-License-Identifier: AGPL-3.0-only
// Two processes opening one fresh state at once (state-store.test.ts:156-169; rule 2.6, trap #1785): the database is
// made WAL on a file of the builder's own before it takes the name, so neither start fails, ten times in ten. The
// children are this test binary run again on the one test below, in their own process groups.
mod common;

use std::os::unix::process::CommandExt;
use std::path::PathBuf;
use std::process::{Command, Stdio};

use common::*;
use wsp_store::sqlite_store::sqlite_store;
use wsp_store::store::Store;

const STATE_ENV: &str = "WSP_STORE_RACE_STATE";

/// One start: a store over the state the parent names (a fresh home of its own when run alone), asked for its keys.
#[test]
fn a_start_over_no_state_file_opens_one_and_answers_no_keys() {
    let alone = Home::new();
    let state = std::env::var(STATE_ENV).map(PathBuf::from).unwrap_or_else(|_| alone.state());
    let store = sqlite_store(&state, writer());
    let keys = run(store.keys("workspaces")).unwrap();
    println!("\nKEYS {}", serde_json::to_string(&keys).unwrap());
    assert_eq!(keys, Vec::<String>::new());
}

fn start(state: &std::path::Path) -> std::process::Child {
    Command::new(std::env::current_exe().unwrap())
        .args(["--exact", "a_start_over_no_state_file_opens_one_and_answers_no_keys", "--nocapture", "--test-threads=1"])
        .env(STATE_ENV, state)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .process_group(0)
        .spawn()
        .unwrap()
}

#[test]
fn two_starts_at_once_over_no_state_file_make_one_database_ten_times_in_ten() {
    let home = Home::new();
    for round in 0..10 {
        for name in home.names() {
            std::fs::remove_file(home.dir.path().join(name)).unwrap();
        }
        let (one, two) = (start(&home.state()), start(&home.state()));
        for (which, child) in [("one", one), ("two", two)] {
            let out = child.wait_with_output().unwrap();
            let said = format!("{}{}", String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr));
            assert!(out.status.success(), "round {round}, start {which} failed:\n{said}");
            assert!(said.lines().any(|l| l == "KEYS []"), "round {round}, start {which}:\n{said}");
        }
        let left: Vec<String> = home.names().into_iter().filter(|n| n != "state.db-wal" && n != "state.db-shm").collect();
        assert_eq!(left, vec!["state.db"], "round {round}");
    }
}
