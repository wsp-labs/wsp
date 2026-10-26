// SPDX-License-Identifier: AGPL-3.0-only
//! A tape whose answers read the computer the host runs on (wsp recipe scan, wsp setup, wsp doctor)
//! agrees only on the computer it was recorded on. Its meta names that computer, and a run elsewhere that differs says
//! where it was recorded rather than failing as a port would.

use serde_json::json;
use std::fs;
use std::process::Command;
use xtask::check::{check, record, Options};
use xtask::host::{Host, Replay};
use xtask::normalize::Facts;
use xtask::seed::Order;
use xtask::tape::Tape;

/// The same host on another computer: the tools it finds installed are that computer's.
struct On(&'static [&'static str]);

impl Host for On {
    fn name(&self) -> String {
        "a host".into()
    }

    fn replay(&self, tape: &Tape, _: &mut Order) -> Result<Replay, String> {
        let replies = tape.ins.iter().map(|f| json!({ "id": f["id"], "ok": true, "installed": self.0 })).collect();
        Ok(Replay { replies, events: Vec::new(), facts: Facts::default() })
    }
}

#[test]
fn a_tape_read_off_one_computer_says_where_when_another_answers_otherwise() {
    let dir = tempfile::tempdir().unwrap();
    assert!(Command::new("git").args(["init", "-q"]).current_dir(dir.path()).status().unwrap().success());
    let tape = dir.path().join("parity/tapes/setup/here.tape");
    fs::create_dir_all(tape.parent().unwrap()).unwrap();
    let meta = json!({ "meta": { "door": "ws", "computer": "aws-mumbai" } });
    fs::write(&tape, format!("{meta}\n{}\n", json!({ "in": { "id": 1, "op": "init.get" } }))).unwrap();
    let ops = vec!["init.get".to_string()];
    record(dir.path(), &On(&["tmux", "pnpm"]), &Options { jobs: 1, ..Options::default() }, &ops).unwrap();
    let said = check(dir.path(), &On(&["tmux"]), &Options { jobs: 1, ..Options::default() }).unwrap().render();
    assert!(said.contains("aws-mumbai"), "{said}");
}
