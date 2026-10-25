// SPDX-License-Identifier: AGPL-3.0-only
//! `proc_name` against node: 1,500 generated command lines (multi-byte words, versions, schemes, `git+`, scoped and
//! versioned packages, line breaks in a word) with the name `procName` of the protocol bundle answered for each.

use serde_json::{json, Value};
use wsp_frames::ProcEntry;
use wsp_wire::views::activity_names::proc_name;

fn entry(comm: &str, cmdline: &str) -> ProcEntry {
    serde_json::from_value(json!({ "pid": 1, "ppid": 0, "user": "u", "state": "S", "comm": comm, "cmdline": cmdline, "cpu": 0, "rss": 0, "startedAt": 0 }))
        .unwrap()
}

#[test]
fn proc_name_names_every_generated_command_line_as_node_names_it() {
    let rows: Vec<Value> = serde_json::from_str(include_str!("fixtures/proc_names.json")).unwrap();
    assert_eq!(rows.len(), 1500);
    let wrong: Vec<String> = rows
        .iter()
        .filter_map(|row| {
            let [comm, cmdline, want] = [0, 1, 2].map(|i| row[i].as_str().unwrap());
            let got = proc_name(&entry(comm, cmdline));
            (got != want).then(|| format!("{comm:?} {cmdline:?}: node {want:?}, here {got:?}"))
        })
        .collect();
    assert!(wrong.is_empty(), "{} of 1500 differ, first: {:?}", wrong.len(), wrong.first());
}

#[test]
fn a_newline_before_the_version_keeps_the_version_as_node_does() {
    assert_eq!(proc_name(&entry("npx", "npx a\n@1")), "a\n@1");
    assert_eq!(proc_name(&entry("npx", "npx a@1")), "a");
}
