// SPDX-License-Identifier: AGPL-3.0-only
//! The answers a spec example writes down are read as a replay reads its own host's, so an example outlives a bump of
//! the TS tree and says nothing of the folder it was recorded in: the version of its `auth` answer, the temp folder
//! of the recording, and an answer to a frame the example sent as text, which no request of it names.

use serde_json::{json, Value};
use std::fs;
use std::process::Command;
use xtask::check::{check, record, Options};
use xtask::host::{Host, Replay};
use xtask::normalize::Facts;
use xtask::seed::Order;
use xtask::tape::Tape;

/// A host on version 0.3.9, run in `/tmp/wsp-run-1`, that answers a frame sent as text with an id of its own.
struct Host93;

impl Host for Host93 {
    fn name(&self) -> String {
        "a host of 0.3.9".into()
    }

    fn replay(&self, _: &Tape, _: &mut Order) -> Result<Replay, String> {
        let mut facts = Facts::default();
        facts.add("0.3.9", "<version>");
        facts.add_folder("/tmp/wsp-run-1", "<tmp>");
        Ok(Replay {
            replies: vec![
                json!({ "id": 1, "ok": true, "version": "0.3.9", "road": "up" }),
                json!({ "id": 2, "ok": false, "error": "/tmp/wsp-run-1/home is outside the folders wsp browses" }),
            ],
            events: vec![json!({ "id": 0, "ok": true, "projects": [] })],
            facts,
        })
    }
}

#[test]
fn a_spec_example_written_on_another_version_and_in_another_folder_agrees_with_a_replay() {
    let dir = tempfile::tempdir().expect("temp");
    assert!(Command::new("git").args(["init", "-q"]).current_dir(dir.path()).status().expect("git").success());
    let spec = dir.path().join("docs/rust-host-spec/x.md");
    fs::create_dir_all(spec.parent().unwrap()).unwrap();
    let frames: [Value; 6] = [
        json!({ "in": { "id": 1, "op": "auth" } }),
        json!({ "out": { "id": 1, "ok": true, "version": "0.3.8", "road": "up" } }),
        json!({ "in": { "id": 2, "op": "host.folders" } }),
        json!({ "out": { "id": 2, "ok": false, "error": "/tmp/wsp-tape-AbC123/home is outside the folders wsp browses" } }),
        json!({ "in": "{\"id\":-0,\"op\":\"projects.list\"}" }),
        json!({ "out": { "id": 0, "ok": true, "projects": [] } }),
    ];
    let body: Vec<String> = frames.iter().map(Value::to_string).collect();
    fs::write(&spec, format!("```tape\n{}\n```\n", body.join("\n"))).unwrap();

    let ops = vec!["auth".to_string(), "host.folders".to_string(), "projects.list".to_string()];
    let only = Options { jobs: 1, only: Some("spec/x/1".into()), ..Options::default() };
    record(dir.path(), &Host93, &only, &ops).expect("record");
    let report = check(dir.path(), &Host93, &only).expect("check");
    assert!(report.ok(), "{}", report.render());
    assert!(report.render().contains("result: 1 passed, 0 accepted, 0 failed"), "{}", report.render());
}
