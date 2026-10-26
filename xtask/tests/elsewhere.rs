// SPDX-License-Identifier: AGPL-3.0-only
//! A check on a computer other than the one the tapes that read the computer were recorded on: it skips those tapes,
//! says where they were recorded, and passes when every tape it ran passed. Coverage counts only the tapes it ran.

use serde_json::{json, Value};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use xtask::check::{check, record, Options, Report};
use xtask::host::{Host, Replay};
use xtask::normalize::Facts;
use xtask::seed::Order;
use xtask::tape::{Door, Tape};

/// The repository's own tapes and goldens in a temp repo, the tapes marked as recorded on this computer marked as
/// recorded on another.
fn the_tapes_recorded_elsewhere() -> tempfile::TempDir {
    fn copy(from: &Path, to: &Path) {
        fs::create_dir_all(to).unwrap();
        for entry in fs::read_dir(from).unwrap().flatten() {
            let (src, dst) = (entry.path(), to.join(entry.file_name()));
            if src.is_dir() {
                copy(&src, &dst);
            } else if src.extension().is_some_and(|e| e == "tape") {
                let text = fs::read_to_string(&src).unwrap();
                fs::write(&dst, text.replace(&format!("\"computer\":\"{}\"", xtask::computer::name()), "\"computer\":\"far-box\""))
                    .unwrap();
            } else {
                fs::copy(&src, &dst).unwrap();
            }
        }
    }
    let dir = tempfile::tempdir().unwrap();
    assert!(Command::new("git").args(["init", "-q"]).current_dir(dir.path()).status().unwrap().success());
    let parity = Path::new(env!("CARGO_MANIFEST_DIR")).join("../parity");
    for part in ["tapes", "goldens"] {
        copy(&parity.join(part), &dir.path().join("parity").join(part));
    }
    for file in ["accepted.tsv", "units.tsv"] {
        fs::copy(parity.join(file), dir.path().join("parity").join(file)).unwrap();
    }
    dir
}

/// A host that answers every tape with its golden, as the TS host does.
struct Goldens(PathBuf);

impl Host for Goldens {
    fn name(&self) -> String {
        "the goldens".into()
    }

    fn replay(&self, tape: &Tape, _: &mut Order) -> Result<Replay, String> {
        let golden: Value =
            serde_json::from_str(&fs::read_to_string(self.0.join(format!("{}.json", tape.id))).map_err(|e| e.to_string())?).unwrap();
        let list = |k: &str| golden[k].as_array().cloned().unwrap_or_default();
        Ok(Replay { replies: list("replies"), events: list("events"), facts: Facts::default() })
    }
}

fn checked_elsewhere() -> Report {
    let dir = the_tapes_recorded_elsewhere();
    check(dir.path(), &Goldens(dir.path().join("parity/goldens")), &Options { jobs: 3, ..Options::default() }).unwrap()
}

#[test]
fn a_check_on_another_computer_passes_when_every_tape_it_ran_passed() {
    let report = checked_elsewhere();
    assert_eq!(report.failed(), 0, "{}", report.render());
    assert!(report.skipped() > 0);
    assert!(report.ok(), "every tape it ran passed, and the run fails: {:?}", report.empty_doors());
}

#[test]
fn a_family_whose_every_tape_was_skipped_reads_as_a_family_with_no_tape() {
    let report = checked_elsewhere();
    // folder.make is sent only by tapes that read the computer (wsp setup's folder step).
    assert!(report.empty_families().contains(&"folder".to_string()), "families with no tape run: {:?}", report.empty_families());
}

/// A host that answers every request with a plain yes.
struct Yes;

impl Host for Yes {
    fn name(&self) -> String {
        "yes".into()
    }

    fn replay(&self, tape: &Tape, _: &mut Order) -> Result<Replay, String> {
        let replies =
            tape.ins.iter().filter(|f| f.get("id").is_some_and(|i| !i.is_null())).map(|f| json!({ "id": f["id"], "ok": true })).collect();
        Ok(Replay { replies, events: Vec::new(), facts: Facts::default() })
    }
}

#[test]
fn a_narrowed_check_that_skipped_every_tape_it_chose_does_not_pass() {
    let dir = tempfile::tempdir().unwrap();
    assert!(Command::new("git").args(["init", "-q"]).current_dir(dir.path()).status().unwrap().success());
    let tape = dir.path().join("parity/tapes/ws/there.tape");
    fs::create_dir_all(tape.parent().unwrap()).unwrap();
    fs::write(
        &tape,
        format!("{}\n{}\n", json!({ "meta": { "door": "ws", "computer": "far-box" } }), json!({ "in": { "id": 1, "op": "init.get" } })),
    )
    .unwrap();
    record(dir.path(), &Yes, &Options { jobs: 1, ..Options::default() }, &["init.get".to_string()]).unwrap();
    let report = check(dir.path(), &Yes, &Options { jobs: 1, only: Some("there".into()), ..Options::default() }).unwrap();
    assert!(!report.ok(), "nothing ran and the run passes:\n{}", report.render());
}

/// A host whose recipe answers print the table of what this computer has installed, with sizes.
struct Recipe;

impl Host for Recipe {
    fn name(&self) -> String {
        "recipe".into()
    }

    fn replay(&self, tape: &Tape, _: &mut Order) -> Result<Replay, String> {
        let table = "Agents\n○  Codex         installed  installed here, never used        455 MB\n";
        let replies = match tape.door {
            Door::Cli => vec![json!({ "exit": 0, "stdout": table, "stderr": "" })],
            _ => vec![
                json!({ "jsonrpc": "2.0", "id": 1, "result": { "structuredContent": { "agents": [{ "id": "codex", "why": "installed here, never used", "size": 477102080 }] } } }),
            ],
        };
        Ok(Replay { replies, events: Vec::new(), facts: Facts::default() })
    }
}

#[test]
fn a_recipe_answer_that_lists_what_this_computer_has_installed_marks_its_tape() {
    // recorded/cli/skill/wsp-recipe-tick-used and recorded/mcp/mcp-record/recipe-no-input print this box's sizes.
    for (door, frame) in [
        ("cli", json!({ "argv": ["recipe", "--tick", "used"] })),
        ("mcp", json!({ "jsonrpc": "2.0", "id": 1, "method": "tools/call", "params": { "name": "recipe", "arguments": {} } })),
    ] {
        let dir = tempfile::tempdir().unwrap();
        assert!(Command::new("git").args(["init", "-q"]).current_dir(dir.path()).status().unwrap().success());
        let tape = dir.path().join("parity/tapes/t/recipe.tape");
        fs::create_dir_all(tape.parent().unwrap()).unwrap();
        fs::write(&tape, format!("{}\n{}\n", json!({ "meta": { "door": door } }), json!({ "in": frame }))).unwrap();
        record(dir.path(), &Recipe, &Options { jobs: 1, ..Options::default() }, &[]).unwrap();
        let first = fs::read_to_string(&tape).unwrap();
        assert!(first.lines().next().unwrap().contains("\"computer\""), "the {door} recipe tape is not marked: {first}");
    }
}
