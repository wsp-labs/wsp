// SPDX-License-Identifier: AGPL-3.0-only
//! The harness held to its own promises with a host of its own making behind the one interface: a golden edited by
//! hand fails the run naming the tape and the field, a family with no tape fails it, every tape block of a spec file
//! is a tape, and a seeded run that breaks says its seed.

use serde_json::{json, Value};
use std::fs;
use std::path::Path;
use std::process::Command;
use xtask::check::{check, record, Options};
use xtask::host::{Host, Replay};
use xtask::normalize::{canonical, Facts};
use xtask::seed::Order;
use xtask::tape::{self, Door, Tape};

/// A host that answers each frame by its op, and answers wrong once more than one frame is out at a time.
struct Fake;

impl Host for Fake {
    fn name(&self) -> String {
        "the fake host".into()
    }

    fn replay(&self, tape: &Tape, order: &mut Order) -> Result<Replay, String> {
        let mut replies = Vec::new();
        let mut i = 0;
        while i < tape.ins.len() {
            let window = order.window().min(tape.ins.len() - i);
            for frame in &tape.ins[i..i + window] {
                let op = frame["op"].as_str().unwrap_or("");
                replies.push(json!({ "id": frame["id"], "ok": window == 1, "asked": op, "at": "2026-10-25T09:00:00.000Z", "workspaceId": "ws_1a2b3c4d" }));
            }
            i += window;
        }
        Ok(Replay { replies, events: vec![json!({ "type": "status.changed" })], facts: Facts::default() })
    }
}

const OPS: &[&str] = &["auth", "workspaces.list", "workspaces.get", "places.list"];

fn repo(tapes: &[(&str, &[&str])]) -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("temp");
    assert!(Command::new("git").args(["init", "-q"]).current_dir(dir.path()).status().expect("git").success());
    for (id, ops) in tapes {
        let mut text = String::from("{\"meta\":{\"door\":\"ws\"}}\n");
        for (n, op) in ops.iter().enumerate() {
            text.push_str(&format!("{}\n", json!({ "in": { "id": n + 1, "op": op } })));
        }
        let path = dir.path().join("parity/tapes").join(format!("{id}.tape"));
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, text).unwrap();
    }
    dir
}

fn recorded(root: &Path) {
    let ops: Vec<String> = OPS.iter().map(|s| s.to_string()).collect();
    let done = record(root, &Fake, &Options { jobs: 2, ..Options::default() }, &ops).expect("record");
    assert!(done.failed.is_empty());
}

#[test]
fn the_recorded_goldens_agree_with_a_replay_of_the_same_host() {
    let root = repo(&[("lists/workspaces", &["auth", "workspaces.list", "workspaces.get"]), ("lists/places", &["auth", "places.list"])]);
    recorded(root.path());
    let report = check(root.path(), &Fake, &Options { jobs: 2, ..Options::default() }).expect("check");
    let text = report.render();
    assert!(report.ok(), "{text}");
    assert!(text.contains("coverage: 4 of 4 host ops have a tape, 4 of them an answer on one"), "{text}");
    assert!(text.contains("result: 2 passed, 0 accepted, 0 failed, 0 families with no tape"), "{text}");
}

#[test]
fn an_edited_golden_fails_the_run_naming_the_tape_and_the_field() {
    let root = repo(&[("lists/workspaces", &["auth", "workspaces.list"]), ("lists/places", &["auth", "places.list"])]);
    recorded(root.path());
    let path = root.path().join("parity/goldens/lists/places.json");
    let edited = fs::read_to_string(&path).unwrap().replace("\"places.list\"", "\"places.lst\"");
    fs::write(&path, edited).unwrap();

    let report = check(root.path(), &Fake, &Options { jobs: 1, ..Options::default() }).expect("check");
    let text = report.render();
    assert!(!report.ok());
    assert!(text.contains("FAIL lists/places replies[1].asked: golden \"places.lst\", live \"places.list\""), "{text}");
    assert!(!text.contains("FAIL lists/workspaces"), "{text}");

    // An accepted line naming that tape and field lets it through and is counted as accepted, not passed.
    fs::write(root.path().join("parity/accepted.tsv"), "lists/places\treplies[*].asked\tthe golden was edited by this test\n").unwrap();
    let report = check(root.path(), &Fake, &Options { jobs: 1, ..Options::default() }).expect("check");
    assert!(report.ok(), "{}", report.render());
    assert!(report.render().contains("result: 1 passed, 1 accepted, 0 failed"), "{}", report.render());
}

#[test]
fn a_family_with_no_tapes_fails_the_run() {
    let root = repo(&[("lists/workspaces", &["auth", "workspaces.list", "workspaces.get"]), ("lists/places", &["auth", "places.list"])]);
    recorded(root.path());
    fs::remove_file(root.path().join("parity/tapes/lists/places.tape")).unwrap();

    let report = check(root.path(), &Fake, &Options { jobs: 1, ..Options::default() }).expect("check");
    let text = report.render();
    assert!(!report.ok());
    assert_eq!(report.failed(), 0);
    assert!(text.contains("FAIL families no tape sends an op of: places"), "{text}");
    assert!(text.contains("no tape yet: places.list"), "{text}");
    assert!(text.contains("note: golden lists/places answers no tape"), "{text}");
}

#[test]
fn every_tape_block_in_a_spec_file_is_read_as_a_tape() {
    let root = repo(&[]);
    let spec = root.path().join("docs/rust-host-spec/workspaces.md");
    fs::create_dir_all(spec.parent().unwrap()).unwrap();
    fs::write(
        &spec,
        concat!(
            "# Workspaces\n\nA list on a fresh host is empty.\n\n",
            "```tape\n",
            "{\"in\": {\"id\": 1, \"op\": \"auth\", \"token\": \"<redacted>\"}}\n",
            "{\"out\": {\"id\": 1, \"ok\": true, \"asked\": \"auth\", \"at\": \"<time>\", \"workspaceId\": \"<ws-1>\"}}\n",
            "{\"in\": {\"id\": 2, \"op\": \"workspaces.list\"}}\n",
            "{\"out\": {\"id\": 2, \"ok\": true, \"asked\": \"workspaces.list\", \"at\": \"<time>\", \"workspaceId\": \"<ws-1>\"}}\n",
            "{\"out\": {\"type\": \"status.changed\"}}\n",
            "```\n\nNot a tape:\n\n```json\n{\"in\": {\"op\": \"nothing\"}}\n```\n\n",
            "```tape\n{\"meta\": {\"door\": \"cli\"}}\n{\"in\": {\"argv\": [\"projects\", \"--json\"]}}\n```\n",
        ),
    )
    .unwrap();

    let tapes = tape::load(root.path()).expect("load");
    let ids: Vec<&str> = tapes.iter().map(|t| t.id.as_str()).collect();
    assert_eq!(ids, ["spec/workspaces/1", "spec/workspaces/2"]);
    assert_eq!(tapes[0].door, Door::Ws);
    assert_eq!(tapes[0].ops(), ["auth", "workspaces.list"]);
    assert_eq!(tapes[0].outs.len(), 3);
    assert_eq!(tapes[1].door, Door::Cli);
    assert_eq!(tapes[1].ins, [json!({ "argv": ["projects", "--json"] })]);

    // The answers a spec block writes down are held to the golden too, so a spec that says what the host does not
    // say fails the run.
    fs::write(&spec, fs::read_to_string(&spec).unwrap().replacen("\"asked\": \"workspaces.list\"", "\"asked\": \"workspaces.lst\"", 1))
        .unwrap();
    let ws_only = Options { jobs: 1, only: Some("spec/workspaces/1".into()), ..Options::default() };
    let ops: Vec<String> = OPS.iter().map(|s| s.to_string()).collect();
    record(root.path(), &Fake, &ws_only, &ops).expect("record");
    let report = check(root.path(), &Fake, &ws_only).expect("check");
    assert!(
        report.render().contains("FAIL spec/workspaces/1 replies[1].asked: golden \"workspaces.list\", the tape writes \"workspaces.lst\""),
        "{}",
        report.render()
    );

    // A block left open is refused with the file named.
    fs::write(&spec, "```tape\n{\"in\": {\"id\": 1, \"op\": \"auth\"}}\n").unwrap();
    let err = tape::load(root.path()).unwrap_err();
    assert!(err.contains("workspaces.md: a tape block is never closed"), "{err}");
}

#[test]
fn a_seeded_replay_that_breaks_says_its_seed_and_the_same_seed_draws_the_same_order() {
    let root = repo(&[("lists/workspaces", &["auth", "workspaces.list", "workspaces.get", "workspaces.list", "workspaces.get"])]);
    recorded(root.path());
    let report = check(root.path(), &Fake, &Options { jobs: 1, seed: Some(42), only: Some("lists".into()), ids: None }).expect("check");
    let text = report.render();
    assert!(!report.ok(), "{text}");
    assert!(text.contains("order: seeded, seed 42"), "{text}");
    assert!(text.contains("seed 42: run again with --seed 42"), "{text}");

    let windows = |seed| {
        let mut o = Order::for_tape(Some(seed), "lists/workspaces");
        (0..16).map(|_| o.window()).collect::<Vec<_>>()
    };
    assert_eq!(windows(42), windows(42));
    assert_ne!(windows(42), windows(43));
    assert!(windows(42).iter().all(|w| (1..=4).contains(w)));
}

#[test]
fn ids_are_numbered_by_first_sight_and_the_run_s_own_facts_are_taken_out() {
    let mut facts = Facts::default();
    facts.add_folder("/tmp/wsp-parity-abc", "<tmp>");
    facts.add("4321", "<port>");
    facts.add("lab-box", "<computer>");
    let replies = [
        json!({ "id": 1, "workspaceId": "ws_9f8e7d6c", "home": "/tmp/wsp-parity-abc/home", "at": 1_761_382_800_000_u64, "name": "lab-box" }),
        json!({ "id": 2, "line": "ws_9f8e7d6c and 123e4567-e89b-12d3-a456-426614174000 at 2026-10-25T09:00:00Z", "diskFreeBytes": 7 }),
        json!({ "id": 3, "hostPublicKey": "MCowBQYDK2VwAyEAspCWzRXZPeML+erN7EBimqx0zAKlhQt0JqljKMWhAWM=", "bin": "/usr/local/lib/node_modules/wsp/dist/bin.js" }),
        json!({
            "id": 4,
            "line": "wsp join http://127.0.0.1:38729 --code 6K1A-YZD5.SHA256:4V/2FMknEFkP6jIF7509y/JUeWWkhyIlp1bnAsKCXeI",
            "code": "BYB7YQKX",
            "port": 38753,
            "ssh": { "port": 2222, "url": "http://127.0.0.1:4400" },
            "exit": { "code": 0 },
            "warn": "(node:257409) ExperimentalWarning",
            "defaults": { "pr_1a9e73bb": 1 },
            "memoryDir": "/x/-tmp-wsp-parity-abc-scratch/memory",
            "ephemeral": "YAXNDdzLsQUIvQVHrmiGLyab+fuchUyNDr+xfMlDgXs=",
            "table": "here  8      31 GB   163.1 GB   none\nup 502ms",
            "doctor": "local workspace  wsp-doctor-ywv1Rp (made for this run) and wsp-parity-harness stays at 127.0.0.1:32923",
            "stdout": "{\"diskFreeBytes\":1,\"name\":\"lab-box\"}\n{\"ok\":true}\n",
            "text": "{\n  \"diskFreeBytes\": 2\n}",
            "placeId": "p_59d14321ab",
        }),
    ];
    let got = canonical(&replies, &[json!({ "workspaceId": "ws_00000000" })], &facts);
    assert_eq!(
        got,
        json!({
            "replies": [
                { "id": 1, "workspaceId": "<ws-1>", "home": "<tmp>/home", "at": "<time>", "name": "<computer>" },
                { "id": 2, "line": "<ws-1> and <uuid-1> at <time>", "diskFreeBytes": "<machine>" },
                { "id": 3, "hostPublicKey": "<id-1>", "bin": "/usr/local/lib/node_modules/wsp/dist/bin.js" },
                {
                    "id": 4,
                    "line": "wsp join http://127.0.0.1:<port> --code <id-2>.<id-3>",
                    "code": "<code>",
                    "port": "<port>",
                    "ssh": { "port": 2222, "url": "http://127.0.0.1:4400" },
                    "exit": { "code": 0 },
                    "warn": "(node:<pid>) ExperimentalWarning",
                    "defaults": { "<pr-1>": 1 },
                    "memoryDir": "/x/<tmp-dashed>-scratch/memory",
                    "ephemeral": "<id-4>",
                    "table": "here  8  <reading>  <reading>  none\nup <reading>",
                    "doctor": "local workspace  wsp-doctor-<temp> (made for this run) and wsp-parity-harness stays at 127.0.0.1:<port>",
                    "stdout": { "jsonLines": [{ "diskFreeBytes": "<machine>", "name": "<computer>" }, { "ok": true }] },
                    "text": { "json": { "diskFreeBytes": "<machine>" } },
                    "placeId": "<p-1>",
                },
            ],
            "events": [{ "workspaceId": "<ws>" }],
        })
    );
    let _: Value = got;
}

#[test]
fn keys_in_another_order_are_a_difference_named_by_their_object() {
    let golden: Value = serde_json::from_str(r#"{"replies": [{"id": 2, "terminalZoom": {"zeta": 2, "alpha": 1}}]}"#).unwrap();
    let sorted: Value = serde_json::from_str(r#"{"replies": [{"id": 2, "terminalZoom": {"alpha": 1, "zeta": 2}}]}"#).unwrap();
    let found = xtask::diff::diff(&golden, &sorted);
    assert_eq!(found.len(), 1, "{found:?}");
    assert_eq!(found[0].field, "replies[0].terminalZoom (key order)");
    assert_eq!((found[0].golden.as_str(), found[0].live.as_str()), (r#"["zeta","alpha"]"#, r#"["alpha","zeta"]"#));
    // Keys one side lacks are their own difference and leave the order of the rest to be read.
    let fewer: Value = serde_json::from_str(r#"{"replies": [{"id": 2, "terminalZoom": {"zeta": 2}}]}"#).unwrap();
    assert_eq!(xtask::diff::diff(&golden, &fewer).iter().map(|d| d.field.as_str()).collect::<Vec<_>>(), ["replies[0].terminalZoom.alpha"]);
}

fn tapes_of(families: &[(&str, usize)], shadow: usize) -> Vec<Tape> {
    let mut out = Vec::new();
    for (family, n) in families {
        for i in 0..*n {
            out.push(
                Tape::parse(
                    &format!("recorded/{family}-{i}"),
                    &format!("{}\n{}\n", json!({"in": {"id": 1, "op": "auth"}}), json!({"in": {"id": 2, "op": format!("{family}.list")}})),
                    "test",
                    1,
                )
                .unwrap(),
            );
        }
    }
    for i in 0..shadow {
        out.push(
            Tape::parse(&format!("shadow/day-{i}"), &format!("{}\n", json!({"in": {"id": 1, "op": "workspaces.list"}})), "test", 1)
                .unwrap(),
        );
    }
    out
}

#[test]
fn every_shadow_tape_and_one_in_five_of_each_family_is_held_back_by_the_seed() {
    let mut tapes = tapes_of(&[("workspaces", 20), ("places", 10), ("slates", 3)], 4);
    for id in ["hand/key-order", "spec/wire/1", "spec/wire/2"] {
        tapes.push(Tape::parse(id, &format!("{}\n", json!({"in": {"id": 1, "op": "preferences.get"}})), "test", 1).unwrap());
    }
    let held = xtask::held::held(&tapes, 2443);
    assert!(!held.iter().any(|id| id.starts_with("hand/") || id.starts_with("spec/")), "{held:?}");
    assert!((0..4).all(|i| held.contains(&format!("shadow/day-{i}"))));
    let of = |family: &str| held.iter().filter(|id| id.starts_with(&format!("recorded/{family}-"))).count();
    assert_eq!((of("workspaces"), of("places"), of("slates")), (4, 2, 1));
    assert_eq!(held, xtask::held::held(&tapes, 2443));
    assert_ne!(held, xtask::held::held(&tapes, 2444));
}

fn unit_repo() -> (tempfile::TempDir, xtask::unit::Unit) {
    let root = repo(&[("lists/workspaces", &["auth", "workspaces.list"]), ("lists/places", &["auth", "places.list"])]);
    let p = root.path();
    fs::create_dir_all(p.join("host/src/workspaces")).unwrap();
    fs::create_dir_all(p.join("host/tests")).unwrap();
    fs::write(p.join("host/src/workspaces/mod.rs"), "pub fn list() -> Vec<String> {\n    todo!()\n}\n").unwrap();
    fs::write(p.join("host/tests/workspaces.rs"), "#[test]\nfn lists() {}\n").unwrap();
    fs::write(p.join("parity/units.tsv"), "# unit\tmanifest\tpackage\tsrc\ttests\tfilter\tops\nworkspaces\thost/Cargo.toml\twsp-host\thost/src/workspaces\thost/tests/workspaces.rs\t-\tworkspaces.*,worktree.*\n").unwrap();
    let git = |args: &[&str]| assert!(Command::new("git").args(args).current_dir(p).status().unwrap().success());
    git(&["add", "-A"]);
    git(&["-c", "user.name=lab", "-c", "user.email=lab@example.com", "commit", "-qm", "packet"]);
    let unit = xtask::unit::read_units(p).unwrap().remove(0);
    (root, unit)
}

#[test]
fn a_unit_is_refused_a_changed_test_a_changed_tape_a_path_outside_it_and_a_stub_left_in_it() {
    let (root, unit) = unit_repo();
    let p = root.path();
    assert_eq!(unit.ops, ["workspaces.*", "worktree.*"]);
    let refusals = || xtask::unit::refusals(p, &unit, &xtask::unit::changed(p, "HEAD").unwrap());
    assert_eq!(refusals(), ["host/src/workspaces/mod.rs:2: todo!() is left in the unit"]);

    fs::write(
        p.join("host/src/workspaces/mod.rs"),
        "pub fn list() -> Vec<String> {\n    Vec::new()\n}\npub fn get() {\n    unimplemented!()\n}\n",
    )
    .unwrap();
    fs::write(p.join("host/tests/workspaces.rs"), "#[test]\nfn lists() { assert!(true) }\n").unwrap();
    fs::write(p.join("parity/tapes/lists/workspaces.tape"), "{\"in\": {\"id\": 1, \"op\": \"workspaces.list\"}}\n").unwrap();
    fs::write(p.join("host/src/lib.rs"), "pub mod workspaces;\n").unwrap();
    let mut got = refusals();
    got.sort();
    assert_eq!(
        got,
        [
            "changes a path outside the unit (host/src/workspaces): host/src/lib.rs",
            "changes a test or a tape, which the packet fixed: host/tests/workspaces.rs",
            "changes a test or a tape, which the packet fixed: parity/tapes/lists/workspaces.tape",
            "host/src/workspaces/mod.rs:5: unimplemented!() is left in the unit",
        ]
    );
}

#[test]
fn a_unit_runs_the_tapes_that_send_its_ops_and_never_a_held_back_one() {
    let (root, unit) = unit_repo();
    let tapes = tape::load(root.path()).unwrap();
    let none = std::collections::BTreeSet::new();
    assert_eq!(xtask::unit::visible_tapes(&unit, &tapes, &none).into_iter().collect::<Vec<_>>(), ["lists/workspaces"]);
    let held: std::collections::BTreeSet<String> = ["lists/workspaces".to_string()].into();
    assert!(xtask::unit::visible_tapes(&unit, &tapes, &held).is_empty());
}
