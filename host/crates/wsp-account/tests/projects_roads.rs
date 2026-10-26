// SPDX-License-Identifier: AGPL-3.0-only
//! The deadline and the scratch folder a move hands the engine's roads (projects.ts:75-92, 540, 681): an upload or a
//! landing is given the folder the machine's kind names and 600 s, as is each download of an export.
#![allow(dead_code, unused_imports)]
#[path = "support/base.rs"]
mod base;
#[path = "support/projects.rs"]
mod projects;

use std::rc::Rc;

use base::{person, record, run};
use projects::{FakeBundler, FakeLander, FakeProjects};
use serde_json::{json, Value};
use wsp_core::types::wiring::{PackedState, ProjectExportOptions, ProjectImportOptions};

const MOVE_DEADLINE_MS: u64 = 600_000;

// projects.ts:75-79 and :87-92: `tmpDir: ctx.moduleOf(kind).scratch(entry), timeoutMs: 600_000` on landBundle and on
// importInto; the engine's own default is 120 s and the shared /tmp.
#[test]
fn the_import_gives_its_uploads_the_kinds_scratch_folder_and_ten_minutes() {
    run(async {
        let fake = FakeProjects::new();
        let area = fake.area();
        let entry = fake.base.hold(record("ws_1", "place", "pr_x", json!({ "name": "task-1" })));
        let plan = json!({ "source": "/Users/dev/code/proj", "repo": true, "files": 6, "bytes": 4321, "secrets": [], "values": [], "excluded": [], "skipped": [], "agents": [{ "agent": "claude", "name": "Claude Code", "sessions": 2, "bytes": 4096, "carry": "moves" }] });
        let bundler = FakeBundler::new(&fake.base, plan);
        let moved = json!({ "agent": "claude", "files": 1, "bytes": 26, "outcome": "moved" });
        *bundler.state.borrow_mut() =
            Some(PackedState { tar: vec![9u8; 64], agents: vec![serde_json::from_value(moved).unwrap()], merges: vec![] });
        fake.base.answer_with("agents_on_machine", vec![json!(["claude"])]);
        let opts = ProjectImportOptions {
            workspace_id: "ws_1".to_owned(),
            source: "/Users/dev/code/proj".to_owned(),
            dest: "/home/dev/proj".to_owned(),
            replace: None,
            carry: None,
            rewrite: None,
            agents: Some(vec!["claude".to_owned()]),
            bundler,
        };
        let report: wsp_core::types::internal::ImportReport = Rc::new(|_stage, _message, _progress| Ok(()));
        area.copy_import(&entry, opts, report).await.unwrap();
        let roads = fake.roads.borrow().clone();
        assert_eq!(roads[0], json!({ "road": "land_bundle", "tmpDir": "/tmp", "timeoutMs": MOVE_DEADLINE_MS }));
        assert_eq!(roads[1], json!({ "road": "import_into", "tmpDir": "/tmp", "timeoutMs": MOVE_DEADLINE_MS }));
    });
}

// projects.ts:540 and :681: `exportFolder(..., { timeoutMs: 600_000 })` and `exportPathsInto(..., { timeoutMs: 600_000 })`.
#[test]
fn the_export_gives_each_download_ten_minutes() {
    run(async {
        let fake = FakeProjects::new();
        let area = fake.area();
        fake.base.hold(record("ws_1", "cloud", "pr_x", json!({ "name": "task-1" })));
        let opts = ProjectExportOptions {
            workspace_id: "ws_1".to_owned(),
            source: "/root/work/proj".to_owned(),
            dest: "/Users/dev/code/proj".to_owned(),
            replace: None,
            agents: None,
            lander: FakeLander::new(&fake.base),
        };
        area.export(opts, Some(person())).await.unwrap();
        let roads = fake.roads.borrow().clone();
        assert_eq!(roads[0], json!({ "road": "export_folder", "timeoutMs": MOVE_DEADLINE_MS }));
    });
}
