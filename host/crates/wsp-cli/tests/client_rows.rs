// SPDX-License-Identifier: AGPL-3.0-only
//! `src/verbs/client.rs`, "Output and the rows the listings print" (client.ts:453-735 at 65f6b89c7): the helpers the
//! TS oracle ran (`tests/fixtures/client.json`, written by `oracle/cli.oracle.ts` "the client's helpers") and the TS
//! tests of the tables (`places.test.ts`, `recipes.test.ts`, `usage-table.test.ts`).
//!
//! Every function under test is a `todo!()` stub, so every test here is red until the section is written.

mod common;

use std::fmt::Debug;

use regex::Regex;
use serde_json::{json, Value};

use common::{fixture, RecordingIo, ScriptedHost};
use wsp_cli::verbs::client::{
    computer_lines, delete_after_asked, dollars_asked, flag_for, host_platform, nap_asked, one_of, place_lines, read_usage,
    recipe_lines, recipe_removed_line, recipe_saved_line, recipe_shown_lines, setting_resets, table, turn_limit_asked,
    usage_table_lines, Dialer, Out, UsageRead,
};
use wsp_wire::types::recipe_file::RecipeView;
use wsp_wire::types::usage::{AccountRow, UsageRange, UsageSplit, UsedAnswer};
use wsp_wire::views::place::PendingComputer;
use wsp_wire::views::place_more::{PlaceSpend, PlaceView};
use wsp_wire::wire::daemon_version::DAEMON_VERSION;
use wsp_words::exit::HostError;

// ---------------------------------------------------------------------------------------------------------------
// Shared builders.
// ---------------------------------------------------------------------------------------------------------------

fn client_fixture() -> Value {
    fixture("client", false)
}

fn from_json<T: serde::de::DeserializeOwned>(v: &Value) -> T {
    serde_json::from_value(v.clone()).unwrap_or_else(|e| panic!("{e}: {v}"))
}

fn views(rows: &[Value]) -> Vec<PlaceView> {
    rows.iter().map(from_json).collect()
}

/// `{...base, ...patch}`.
fn with(base: Value, patch: Value) -> Value {
    let mut merged = base.as_object().unwrap().clone();
    for (key, value) in patch.as_object().unwrap() {
        merged.insert(key.clone(), value.clone());
    }
    Value::Object(merged)
}

/// `line.slice(from, to)` over characters; a line shorter than `from` slices to "".
fn slice(line: &str, from: usize, to: Option<usize>) -> String {
    let chars = line.chars().skip(from);
    match to {
        Some(to) => chars.take(to.saturating_sub(from)).collect(),
        None => chars.collect(),
    }
}

/// `line.slice(header.indexOf(name), header.indexOf(next)).trim()`.
fn column(header: &str, line: &str, name: &str, next: &str) -> String {
    slice(line, header.find(name).unwrap(), Some(header.find(next).unwrap())).trim().to_owned()
}

/// `line.split(/\s{2,}/)`.
fn cells(line: &str) -> Vec<String> {
    Regex::new(r"\s{2,}").unwrap().split(line).map(str::to_owned).collect()
}

fn strs(lines: &[&str]) -> Vec<String> {
    lines.iter().map(|l| (*l).to_owned()).collect()
}

/// The three rows `places.test.ts` builds its tables from.
fn here() -> Value {
    json!({ "id": "here", "kind": "computer", "name": "zingzys-mac", "default": false, "shape": { "cpu": 8, "memMb": 16384 }, "present": true, "takesForks": false })
}

fn box_row() -> Value {
    json!({
        "id": "p_1", "kind": "computer", "name": "box", "default": true, "shape": { "cpu": 4, "memMb": 4096 },
        "diskFreeBytes": 831u64 * 1024 * 1024 * 1024, "present": true, "lastSeenAt": "2026-09-12T00:00:00.000Z", "takesForks": true
    })
}

fn solari() -> Value {
    json!({ "id": "solari", "kind": "provider", "name": "solari", "default": false, "rateUsdPerHour": 0.018, "takesForks": true })
}

fn boat() -> Value {
    json!({ "id": "box", "kind": "provider", "name": "box", "default": false, "rateUsdPerHour": 0.16, "takesForks": true })
}

fn running_job() -> Value {
    json!({ "state": "running", "addId": "a_1", "startedAt": "2026-10-03T10:01:00.000Z", "steps": [{ "step": "agents", "state": "running" }], "waiting": [] })
}

// ---------------------------------------------------------------------------------------------------------------
// The fixtures the TS oracle wrote.
// ---------------------------------------------------------------------------------------------------------------

/// What the TS oracle's `caught` recorded for an error: the failure's words, class, exit code, kind and fix.
fn assert_refusal(row: &str, got: Result<impl Debug, HostError>, err: &Value) {
    let e = match got {
        Ok(v) => panic!("{row}: wanted the refusal {:?}, got {v:?}", err["error"]),
        Err(e) => e,
    };
    assert_eq!(e.message, err["error"].as_str().unwrap(), "{row}: message");
    assert_eq!(e.class().as_str(), err["class"].as_str().unwrap(), "{row}: class");
    assert_eq!(i64::from(e.exit_code()), err["exit"].as_i64().unwrap(), "{row}: exit code");
    assert_eq!(e.kind.as_ref().map(|k| k.as_str()), err["kind"].as_str(), "{row}: kind");
    assert_eq!(e.fix.as_deref(), err["fix"].as_str(), "{row}: fix");
}

/// Every row of `asked.<section>`: the word, then either the number JS answered or the refusal it threw. Numbers
/// compare as `f64`, which is what a JS Number is.
fn assert_numbers(section: &str, call: impl Fn(&str) -> Result<f64, HostError>) {
    let fixture = client_fixture();
    let rows = fixture["asked"][section].as_array().unwrap();
    assert!(rows.len() >= 24, "{section}: the fixture lost its rows");
    for row in rows {
        let word = row["word"].as_str().unwrap();
        let label = format!("{section} {word:?}");
        let got = call(word);
        match (row["out"].get("ok"), row["out"].get("err")) {
            (Some(ok), _) => {
                let got = got.unwrap_or_else(|e| panic!("{label}: refused with {:?}", e.message));
                assert_eq!(got, ok.as_f64().unwrap(), "{label}: value");
            }
            (None, Some(err)) => assert_refusal(&label, got, err),
            (None, None) => panic!("{label}: the fixture row says neither ok nor err"),
        }
    }
}

#[test]
fn nap_asked_matches_what_the_ts_answered_for_every_word() {
    assert_numbers("nap", |w| nap_asked(w, "mini").map(|n| n as f64));
}

#[test]
fn turn_limit_asked_matches_what_the_ts_answered_for_every_word() {
    assert_numbers("turnLimit", |w| turn_limit_asked(w, "mini").map(|n| n as f64));
}

#[test]
fn delete_after_asked_matches_what_the_ts_answered_for_every_word() {
    assert_numbers("deleteAfter", |w| delete_after_asked(w, "mini").map(|n| n as f64));
}

#[test]
fn dollars_asked_matches_what_js_number_answered_for_every_word() {
    assert_numbers("dollars", |w| dollars_asked(w, "mini"));
}

#[test]
fn the_limits_the_refusals_name_are_the_ones_the_ts_read_off_its_constants() {
    let fixture = client_fixture();
    let limits = &fixture["asked"]["limits"];
    let nap = nap_asked("181", "mini").unwrap_err().message;
    let turn = turn_limit_asked("25", "mini").unwrap_err().message;
    let delete = delete_after_asked("1441", "mini").unwrap_err().message;
    assert!(nap.contains(&format!("from 1 to {}, or off", limits["napMinutes"])), "{nap}");
    assert!(turn.contains(&format!("from 1 to {}, or off", limits["turnHours"])), "{turn}");
    assert!(delete.contains(&format!("from 0 to {}, and got", limits["deleteMinutes"])), "{delete}");
}

#[test]
fn one_of_matches_what_the_ts_answered_for_every_row() {
    let fixture = client_fixture();
    let rows = fixture["oneOf"].as_array().unwrap();
    assert_eq!(rows.len(), 5, "the fixture lost its rows");
    for row in rows {
        let name = row["name"].as_str().unwrap();
        let words: Vec<&str> = row["words"].as_array().unwrap().iter().map(|w| w.as_str().unwrap()).collect();
        let label = format!("oneOf --{name} {:?} on {}", row["value"], row["on"]);
        let got = one_of(name, &words, row["value"].as_str(), row["on"].as_str());
        match (row["out"].get("ok"), row["out"].get("err")) {
            (Some(ok), _) => {
                let got = got.unwrap_or_else(|e| panic!("{label}: refused with {:?}", e.message));
                assert_eq!(got.as_deref(), ok.as_str(), "{label}: value");
            }
            (None, Some(err)) => assert_refusal(&label, got, err),
            (None, None) => panic!("{label}: the fixture row says neither ok nor err"),
        }
    }
}

#[test]
fn table_matches_the_lines_the_ts_printed_for_every_set_of_rows() {
    let fixture = client_fixture();
    let tables = fixture["tables"].as_array().unwrap();
    assert_eq!(tables.len(), 7, "the fixture lost its tables");
    for (n, case) in tables.iter().enumerate() {
        let rows: Vec<Vec<String>> = case["rows"].as_array().unwrap().iter().map(|r| r.as_array().unwrap().iter().map(|c| c.as_str().unwrap().to_owned()).collect()).collect();
        let want: Vec<String> = case["lines"].as_array().unwrap().iter().map(|l| l.as_str().unwrap().to_owned()).collect();
        assert_eq!(table(&rows), want, "table {n}: {:?}", case["rows"]);
    }
}

// The cell widths are counted in UTF-16 units, as the TS `cell.length` counts them: an emoji is two, a letter with
// an accent is one.
#[test]
fn table_pads_by_utf16_units_and_never_pads_the_last_cell() {
    let rows = vec![strs(&["NAME", "LAST"]), strs(&["é", "x"]), strs(&["日本", "y"]), strs(&["😀", "z"])];
    assert_eq!(table(&rows), strs(&["NAME  LAST", "é     x", "日本    y", "😀    z"]));
}

#[test]
fn formatter_without_json_prints_the_line_and_streams_the_text() {
    let fixture = client_fixture();
    let want = &fixture["formatter"].as_array().unwrap()[..2];
    assert_eq!(want, [json!("stream:text"), json!("log:the line")], "the fixture changed");
    let io = RecordingIo::new();
    let out = Out::formatter(io.clone(), false);
    out.emit(&json!({ "a": 1, "\u{85}": "\u{9f}" }), Some("the line"));
    out.emit(&json!({ "b": 2 }), None);
    out.stream("text");
    assert_eq!(io.streamed(), strs(&["text"]));
    assert_eq!(io.lines(), strs(&["the line"]));
    assert_eq!(io.errors(), Vec::<String>::new());
}

// A script that prints the answer hands every C1 control to a terminal that reads it, so the line carries them as
// the six characters `\u0085`, and nothing is streamed.
#[test]
fn formatter_with_json_prints_each_value_as_one_escaped_line_and_streams_nothing() {
    let fixture = client_fixture();
    let want: Vec<String> = fixture["formatter"].as_array().unwrap()[2..].iter().map(|l| l.as_str().unwrap().strip_prefix("log:").unwrap().to_owned()).collect();
    assert_eq!(want, vec![r#"{"a":1,"\u0085":"\u009f"}"#.to_owned(), r#"{"b":2}"#.to_owned()], "the fixture changed");
    let io = RecordingIo::new();
    let out = Out::formatter(io.clone(), true);
    out.emit(&json!({ "a": 1, "\u{85}": "\u{9f}" }), Some("the line"));
    out.emit(&json!({ "b": 2 }), None);
    out.stream("text");
    assert_eq!(io.lines(), want);
    assert_eq!(io.streamed(), Vec::<String>::new());
    assert_eq!(io.errors(), Vec::<String>::new());
}

#[test]
fn formatter_prints_the_whole_run_as_the_ts_did() {
    let fixture = client_fixture();
    let mut got: Vec<String> = Vec::new();
    for json in [false, true] {
        let io = RecordingIo::new();
        let out = Out::formatter(io.clone(), json);
        out.emit(&json!({ "a": 1, "\u{85}": "\u{9f}" }), Some("the line"));
        out.emit(&json!({ "b": 2 }), None);
        out.stream("text");
        got.extend(io.streamed().into_iter().map(|t| format!("stream:{t}")));
        got.extend(io.lines().into_iter().map(|l| format!("log:{l}")));
    }
    let want: Vec<String> = fixture["formatter"].as_array().unwrap().iter().map(|l| l.as_str().unwrap().to_owned()).collect();
    assert_eq!(got, want);
}

#[test]
fn flag_for_names_the_computer_only_where_the_line_names_one() {
    let fixture = client_fixture();
    let rows = fixture["misc"]["flagFor"].as_array().unwrap();
    assert_eq!(rows.len(), 2, "the fixture lost its rows");
    for row in rows {
        let flag = row[0].as_str().unwrap();
        assert_eq!(flag_for(flag, row[1].as_str()), row[2].as_str().unwrap(), "flagFor {flag} on {}", row[1]);
    }
}

#[test]
fn setting_resets_are_the_words_the_ts_listed_in_its_order() {
    let fixture = client_fixture();
    let want: Vec<String> = fixture["misc"]["settingResets"].as_array().unwrap().iter().map(|w| w.as_str().unwrap().to_owned()).collect();
    assert!(!want.is_empty(), "the fixture lost its words");
    assert_eq!(setting_resets().into_iter().map(str::to_owned).collect::<Vec<_>>(), want);
}

#[test]
fn host_platform_is_the_word_the_ts_gave_on_this_kind_of_computer() {
    let fixture = client_fixture();
    assert_eq!(host_platform(), fixture["misc"]["hostPlatform"].as_str().unwrap());
}

// ---------------------------------------------------------------------------------------------------------------
// places.test.ts: "the table wsp places prints".
// ---------------------------------------------------------------------------------------------------------------

mod the_table_wsp_places_prints {
    use super::*;
    use wsp_words::place_word::{names_place, PlaceRef};
    use wsp_words::views::place_setup_words::setup_word;
    use wsp_words::words::places::agents_cell;
    use wsp_wire::wire::daemon_version::place_daemon_behind;

    fn rows() -> Vec<Value> {
        vec![here(), box_row(), solari()]
    }

    #[test]
    fn carries_the_cores_the_memory_the_free_disk_and_the_presence_with_the_default_mar() {
        let printed = place_lines(&views(&rows()));
        assert!(printed[0].contains("PLACE"), "{printed:?}");
        assert!(printed[0].contains("DISK FREE"), "{printed:?}");
        // Every place on this list forks, so the column that said yes or no said one word forever.
        assert!(!printed[0].contains("WORKSPACES"), "{printed:?}");
        assert!(!printed[0].contains("ENGINE"), "{printed:?}");
        assert!(printed[1].contains("zingzys-mac"), "{printed:?}");
        assert!(printed[2].contains("box"), "{printed:?}");
        assert!(printed[2].contains("default"), "{printed:?}");
        assert!(printed[3].contains("$0.018/hr"), "{printed:?}");
        assert_eq!(printed.iter().filter(|l| l.contains("default")).count(), 1, "{printed:?}");
    }

    // The TS asserts COPIES sits after ENGINE, a column the header no longer has (indexOf is -1), so that line can
    // never fail; here the order is the header's real one, after DISK FREE and before PRESENT.
    #[test]
    fn says_beside_the_engine_how_a_workspace_s_copy_of_a_project_is_made_there_and_not() {
        let printed = place_lines(&views(&[with(box_row(), json!({ "copies": "reflink" })), here(), solari()]));
        // Beside the engine: both are what that computer brings to a workspace rather than what one asked for.
        let header = &printed[0];
        assert!(header.contains("COPIES"), "{printed:?}");
        assert!(header.find("COPIES").unwrap() > header.find("DISK FREE").unwrap(), "{header}");
        assert!(header.find("COPIES").unwrap() < header.find("PRESENT").unwrap(), "{header}");
        assert!(printed[1].contains("reflink"), "{printed:?}");
        // The columns line up: the word sits under its own heading rather than in the one beside it.
        let copies = |line: &str| column(header, line, "COPIES", "PRESENT");
        assert_eq!(copies(&printed[1]), "reflink");
        assert_eq!(copies(&printed[2]), "");
        assert_eq!(copies(&printed[3]), "");
    }

    #[test]
    fn says_how_to_get_one_when_the_host_holds_none() {
        let printed = place_lines(&[]);
        assert!(printed.join("").contains("wsp add prints the join line"), "{printed:?}");
    }

    // The TS also reads `placeNames` over a client here; that is verbs.ts, not this section, and is not ported.
    #[test]
    fn names_a_cloud_by_the_name_the_app_gives_it_boat_and_solari_never_the_id_stored_s() {
        let rows = rows();
        let first = |line: &String| cells(line).remove(0);
        let printed = computer_lines(&views(&[rows[1].clone(), boat(), rows[2].clone()]), "darwin", &[], &[]);
        assert_eq!(printed[1..].iter().map(first).collect::<Vec<_>>(), strs(&["box", "Boat", "Solari"]));
        let provider = |id: &str, name: &str| PlaceRef { id: id.to_owned(), name: name.to_owned(), kind: Some("provider".to_owned()) };
        let boat = provider("box", "box");
        for word in ["Boat", "boat", "BOAT", "box", "BOX"] {
            assert!(names_place(&boat, word), "{word}");
        }
        assert!(!names_place(&boat, "solari"));
        // A joined computer is named by the word its owner gave it, exactly.
        let joined = PlaceRef { id: "p_1".to_owned(), name: "box".to_owned(), kind: Some("computer".to_owned()) };
        assert!(names_place(&joined, "box"));
        assert!(!names_place(&joined, "Box"));
    }

    #[test]
    fn says_on_the_row_what_a_copy_of_the_image_there_is_doing_and_nothing_where_the_co() {
        let printed = place_lines(&views(&[with(rows()[1].clone(), json!({ "build": "building your image · creating the machine" })), rows()[2].clone()]));
        assert!(printed[0].contains("IMAGE"), "{printed:?}");
        assert!(printed[1].contains("building your image · creating the machine"), "{printed:?}");
        assert!(!printed[2].contains("building"), "{printed:?}");
    }

    #[test]
    fn says_on_the_row_that_a_computer_runs_an_older_daemon_than_this_wsp_deploys_in_on() {
        let rows = rows();
        let printed = place_lines(&views(&[
            with(rows[1].clone(), json!({ "daemonVersion": DAEMON_VERSION - 5 })),
            with(rows[0].clone(), json!({ "id": "p_2", "name": "laptop", "daemonVersion": DAEMON_VERSION })),
        ]));
        assert!(printed[0].contains("BEHIND"), "{printed:?}");
        assert!(printed[1].contains(&format!("daemon {}, host {}", DAEMON_VERSION - 5, DAEMON_VERSION)), "{printed:?}");
        // The word is the protocol's own, so this row and the app's table cannot say it two ways.
        assert!(printed[1].contains(&place_daemon_behind(Some(DAEMON_VERSION - 5)).unwrap()), "{printed:?}");
        // A computer on this wsp's own daemon says nothing in that column, and neither does one that never reported.
        assert!(!printed[2].contains("daemon"), "{printed:?}");
        assert!(!place_lines(&views(&[rows[2].clone()]))[1].contains("daemon"));
    }

    #[test]
    fn says_on_the_computers_table_what_the_setup_on_that_computer_is_doing_and_nothing() {
        let rows = rows();
        let job = running_job();
        let with_job = from_json::<PlaceView>(&with(rows[1].clone(), json!({ "setup": job })));
        let printed = computer_lines(&[with_job.clone(), from_json(&rows[0]), from_json(&rows[2])], "darwin", &[], &[]);
        let header = &printed[0];
        assert!(header.contains("TOOLS"), "{printed:?}");
        let tools = |line: &str| slice(line, header.find("TOOLS").unwrap(), Some(header.find("AGENTS").unwrap())).trim().to_owned();
        // The word is the protocol's own, so this table and the app's row cannot say it two ways.
        assert_eq!(tools(&printed[1]), setup_word(with_job.setup.as_ref(), None));
        assert_eq!(tools(&printed[1]), "setting up the agents");
        assert!(printed[1].contains("Setting up"), "{printed:?}");
        assert_eq!(tools(&printed[2]), "");
        assert_eq!(tools(&printed[3]), "");
        // Once it is over the same column says what stands.
        let over = computer_lines(
            &views(&[with(
                rows[1].clone(),
                json!({
                    "setup": with(job, json!({ "state": "done", "steps": [] })),
                    "applied": { "hash": "h", "at": "x", "rows": [{ "id": "agents/codex", "label": "Codex", "outcome": "installed" }] }
                }),
            )]),
            "darwin",
            &[],
            &[],
        );
        assert_eq!(slice(&over[1], over[0].find("TOOLS").unwrap(), Some(over[0].find("AGENTS").unwrap())).trim(), "1 tool ready");
    }

    #[test]
    fn lists_every_add_that_has_not_reached_set_up_below_the_computers_pending_with_how() {
        let choices = json!({ "name": "spoo" });
        let pending: Vec<PendingComputer> = [
            json!({ "id": "a_1", "address": "root@10.0.0.9", "name": "spoo", "step": "choosing", "choices": choices, "startedAt": "x", "placeId": "p_9" }),
            json!({ "id": "a_2", "address": "root@10.0.0.7", "step": "check", "choices": choices, "startedAt": "x", "failed": { "said": "root@10.0.0.7 runs no systemd" } }),
        ]
        .iter()
        .map(from_json)
        .collect();
        let printed = computer_lines(&views(&[here()]), "darwin", &[], &pending);
        let header = &printed[0];
        let state = |line: &str| slice(line, header.find("STATE").unwrap(), Some(header.find("LAST SEEN").unwrap())).trim().to_owned();
        let seen: Vec<(String, String)> = printed[2..].iter().map(|l| (l.split_whitespace().next().unwrap().to_owned(), state(l))).collect();
        assert_eq!(
            seen,
            vec![("spoo".to_owned(), "Pending".to_owned()), ("root@10.0.0.7".to_owned(), "Setup failed".to_owned())],
            "{printed:?}"
        );
        assert!(printed[2].contains("joined; waiting on what goes on it"), "{printed:?}");
    }

    #[test]
    fn reads_a_computer_that_joined_and_is_still_choosing_as_pending_on_its_own_row_its() {
        let joined = box_row();
        let pending: PendingComputer =
            from_json(&json!({ "id": "a_1", "address": "root@10.0.0.9", "step": "floor", "choices": { "name": "spoo" }, "startedAt": "x", "placeId": joined["id"] }));
        let printed = computer_lines(&views(&[joined]), "darwin", &[], &[pending]);
        let header = &printed[0];
        assert_eq!(printed.len(), 2, "{printed:?}");
        assert_eq!(column(header, &printed[1], "STATE", "LAST SEEN"), "Pending");
        assert_eq!(column(header, &printed[1], "TOOLS", "AGENTS"), "joined; the base tools are going on while you choose");
    }

    #[test]
    fn says_on_the_computers_table_which_agents_stand_on_a_computer_at_which_version_an() {
        let rows = rows();
        let spoo: PlaceView = from_json(&with(
            rows[1].clone(),
            json!({
                "agents": ["claude", "codex"],
                "agentVersions": { "claude": "2.1.270 (Claude Code)", "codex": "codex-cli 0.153.0" },
                "signIns": { "claude": "vault-key", "codex": "none" }
            }),
        ));
        let printed = computer_lines(&[spoo.clone(), from_json(&rows[0]), from_json(&rows[2])], "darwin", &[], &[]);
        let header = &printed[0];
        assert!(header.contains("AGENTS"), "{printed:?}");
        let agents = |line: &str| slice(line, header.find("AGENTS").unwrap(), None).trim().to_owned();
        // The cell is the protocol's own, so this table and the app's row cannot say it two ways.
        assert_eq!(agents(&printed[1]), agents_cell(&spoo));
        assert_eq!(agents(&printed[1]), "claude 2.1.270 signed in with a token, codex 0.153.0 not signed in");
        // This computer reports no agent of its own on this row, and neither does a cloud account.
        assert_eq!(agents(&printed[2]), "");
        assert_eq!(agents(&printed[3]), "");
    }

    #[test]
    fn says_nothing_in_the_image_column_across_the_states_of_the_setup_while_the_tools() {
        let rows = rows();
        let job = with(running_job(), json!({ "steps": [{ "step": "skills", "state": "running" }] }));
        let done = json!({
            "setup": with(job.clone(), json!({ "state": "done", "steps": [] })),
            "applied": { "hash": "h", "at": "x", "rows": [{ "id": "agents/codex", "label": "Codex", "outcome": "installed" }] }
        });
        let of = |place: Value| -> (String, String) {
            let printed = computer_lines(&views(&[place]), "darwin", &[], &[]);
            let header = &printed[0];
            (column(header, &printed[1], "IMAGE", "TOOLS"), column(header, &printed[1], "TOOLS", "AGENTS"))
        };
        // A computer that keeps no image says nothing in that column in any state of the job, and the refusal a fork
        // there meets while the job runs is never one of them.
        assert_eq!(of(rows[1].clone()), (String::new(), String::new()));
        assert_eq!(of(with(rows[1].clone(), json!({ "setup": job }))), (String::new(), "setting up the skills".to_owned()));
        assert_eq!(of(with(rows[1].clone(), done)), (String::new(), "1 tool ready".to_owned()));
    }

    #[test]
    fn says_on_the_computers_table_how_many_threads_run_on_a_computer_and_machines_on_a() {
        let rows = rows();
        let printed = computer_lines(
            &views(&[
                with(rows[0].clone(), json!({ "cap": { "threads": 6 }, "running": 1 })),
                with(rows[1].clone(), json!({ "cap": { "threads": 2 }, "running": 2 })),
                with(rows[2].clone(), json!({ "cap": { "machines": 3, "spendPerDayUsd": 10 }, "running": 2 })),
                with(rows[1].clone(), json!({ "id": "p_2", "name": "fresh", "present": false })),
            ]),
            "darwin",
            &[],
            &[],
        );
        let header = &printed[0];
        assert!(header.contains("THREADS"), "{printed:?}");
        let col = |name: &str, next: &str| -> Vec<String> { printed[1..].iter().map(|line| column(header, line, name, next)).collect() };
        assert_eq!(col("THREADS", "MACHINES"), strs(&["1/6", "2/2", "", ""]));
        assert_eq!(col("MACHINES", "STATE"), strs(&["", "", "2/3", ""]));
        // The word is the protocol's own, so this table and the app's row cannot say it two ways.
        assert_eq!(col("STATE", "LAST SEEN"), strs(&["Ready", "Full", "Ready", "no answer"]));
    }

    #[test]
    fn says_on_the_computers_table_what_a_cloud_spent_today_against_its_spend_per_day_n() {
        let rows = rows();
        let places = views(&[
            with(rows[0].clone(), json!({ "cap": { "threads": 6 }, "running": 0 })),
            with(rows[1].clone(), json!({ "cap": { "threads": 2 }, "running": 0 })),
            with(rows[2].clone(), json!({ "cap": { "machines": 3, "spendPerDayUsd": 10 }, "running": 1 })),
        ]);
        let spend = |today_usd: f64| -> Vec<PlaceSpend> { vec![from_json(&json!({ "place": "solari", "todayUsd": today_usd, "monthUsd": 40, "rateUsdPerHour": 0.11 }))] };
        let col = |printed: &[String], name: &str, next: &str| -> Vec<String> { printed[1..].iter().map(|line| column(&printed[0], line, name, next)).collect() };
        let under = computer_lines(&places, "darwin", &spend(2.314), &[]);
        assert_eq!(col(&under, "SPEND", "STATE"), strs(&["", "", "$2.31/$10"]));
        assert_eq!(col(&under, "STATE", "LAST SEEN"), strs(&["Ready", "Ready", "Ready"]));
        let at = computer_lines(&places, "darwin", &spend(10.0), &[]);
        assert_eq!(col(&at, "SPEND", "STATE"), strs(&["", "", "$10.00/$10"]));
        assert_eq!(col(&at, "STATE", "LAST SEEN"), strs(&["Ready", "Ready", "At limit"]));
        // With no spend read the cell is left empty rather than guessed at zero.
        assert_eq!(col(&computer_lines(&places, "darwin", &[], &[]), "SPEND", "STATE"), strs(&["", "", ""]));
    }

    #[test]
    fn says_how_many_forks_a_place_holds_of_how_many_it_takes_and_nothing_there_for_one() {
        let rows = rows();
        let printed = place_lines(&views(&[
            with(rows[1].clone(), json!({ "forks": { "running": 1, "room": 2 } })),
            with(rows[0].clone(), json!({ "id": "p_2", "name": "laptop" })),
        ]));
        assert!(printed[0].contains("FORKS"), "{printed:?}");
        assert!(printed[1].contains("1 of 3"), "{printed:?}");
        assert!(!printed[2].contains("of"), "{printed:?}");
    }
}

// ---------------------------------------------------------------------------------------------------------------
// recipes.test.ts: "wsp recipes show" and the listing in "a recipe as the host hands it out".
// ---------------------------------------------------------------------------------------------------------------

fn builders_file() -> Value {
    json!({ "name": "Builders", "mcp": { "sentry": { "agents": ["codex"] } } })
}

fn recipe(file: Value, machines: &[&str], saved_at: Option<&str>) -> RecipeView {
    let mut view = json!({ "name": "Builders", "slug": "builders", "summary": "1 MCP server", "machines": machines, "file": file });
    if let Some(at) = saved_at {
        view["savedAt"] = json!(at);
    }
    from_json(&view)
}

mod wsp_recipes_show {
    use super::*;

    #[test]
    fn says_the_one_yes_to_copying_the_servers_keys_where_the_recipe_gives_it_and_nothi() {
        let keys = recipe_shown_lines(&recipe(with(builders_file(), json!({ "copyKeys": true })), &[], None), "h");
        assert!(keys.contains(&"  keys     copied with the servers that carry them".to_owned()), "{keys:?}");
        let plain = recipe_shown_lines(&recipe(builders_file(), &[], None), "h");
        assert!(!plain.iter().any(|line| line.starts_with("  keys")), "{plain:?}");
        assert!(plain.contains(&"  mcp      sentry (agents codex)".to_owned()), "{plain:?}");
    }
}

// The TS test starts a host and lists over a socket; the listing it checks on the command line is the part ported,
// over a recipe that was saved on a known day.
mod a_recipe_as_the_host_hands_it_out {
    use super::*;

    #[test]
    fn holds_neither_wsp_s_own_server_nor_the_package_that_installs_the_wsp_command_cou() {
        let listed = recipe(json!({ "name": "default", "mcp": { "linear": { "agents": ["claude"] } }, "clis": { "turbo": { "via": "npm" } } }), &[], Some("2026-10-05T12:00:00.000Z"));
        // The list on the command line says the day each was saved.
        let lines = recipe_lines(&[listed]);
        assert!(Regex::new(r"^RECIPE\s+HOLDS\s+SAVED\s+COMPUTERS$").unwrap().is_match(&lines[0]), "{lines:?}");
        assert!(lines[1].contains("2026-10-05"), "{lines:?}");
    }
}

// ---------------------------------------------------------------------------------------------------------------
// usage-table.test.ts.
// ---------------------------------------------------------------------------------------------------------------

const NOW: f64 = 1_790_676_000_000.0;
const DAY: u64 = 86_400_000;
const SINCE: u64 = 1_789_084_800_000;

fn outside(day: u64) -> u64 {
    if day < 27 {
        0
    } else {
        21_922_166_667
    }
}

fn threads(day: u64) -> u64 {
    if day < 28 {
        0
    } else {
        6_900_000
    }
}

/// `used` of the test: one day, by agent, nothing in it.
fn empty_used() -> Value {
    json!({ "range": "day", "split": "agent", "rows": [], "series": [], "since": NOW, "until": NOW + DAY as f64 })
}

/// `USED_BY_SOURCE` of `packages/protocol/test/fixtures/usage-by-source.ts`.
fn used_by_source() -> Value {
    json!({
        "range": "month",
        "split": "source",
        "rows": [
            { "key": "log", "label": "Outside wsp", "tokens": { "input": 65_641_500_000u64, "output": 125_000_000u64, "cached": 62_400_000_000u64, "cacheWrite": 3_220_000_000u64, "reasoning": 0 }, "costList": 44925.05, "estimate": 44925.05, "saved": 250_000, "priced": true },
            { "key": "wsp", "label": "wsp threads", "tokens": { "input": 13_700_000, "output": 100_000, "cached": 12_900_000, "cacheWrite": 640_000, "reasoning": 0 }, "costReported": 5.07, "costList": 3.56, "estimate": 8.63, "saved": 47, "priced": true, "turns": 21 }
        ],
        "series": (0..30u64).map(|i| json!({ "t": SINCE + i * DAY, "tokens": outside(i) + threads(i) })).collect::<Vec<_>>(),
        "lines": [
            { "key": "log", "label": "Outside wsp", "points": (0..30u64).map(outside).collect::<Vec<_>>() },
            { "key": "wsp", "label": "wsp threads", "points": (0..30u64).map(threads).collect::<Vec<_>>() }
        ],
        "since": SINCE,
        "until": SINCE + 30 * DAY,
        "logs": { "agents": ["Claude Code", "Codex"], "computers": ["zingzy's MacBook Pro"] }
    })
}

/// `USED_BY_ACCOUNT`: the same month by account, the logs outside wsp counted with the threads.
fn used_by_account() -> Value {
    with(
        used_by_source(),
        json!({
            "split": "account",
            "rows": [
                { "key": "claude@here", "label": "Claude Code with an API key", "agent": "claude", "tokens": { "input": 65_647_120_132u64, "output": 125_037_700u64, "cached": 62_405_050_000u64, "cacheWrite": 3_220_440_000u64, "reasoning": 0 }, "costReported": 3.56, "costList": 44925.05, "estimate": 44928.61, "priced": true, "turns": 12 },
                { "key": "codex:acct-1", "label": "Codex with ChatGPT Plus", "agent": "codex", "tokens": { "input": 8_150_000, "output": 50_000, "cached": 7_800_000, "cacheWrite": 0, "reasoning": 0 }, "costList": 7.15, "estimate": 7.15, "priced": true, "turns": 9 },
                { "key": "claude@pl_hetzner", "label": "Claude Code signed in on hetzner", "agent": "claude", "tokens": { "input": 0, "output": 0, "cached": 0, "cacheWrite": 0, "reasoning": 0 }, "costList": 0, "estimate": 0, "priced": true, "turns": 0 }
            ],
            "lines": [
                { "key": "claude@here", "label": "Claude Code with an API key", "points": (0..30u64).map(|i| outside(i) + threads(i)).collect::<Vec<_>>() },
                { "key": "codex:acct-1", "label": "Codex with ChatGPT Plus", "points": (0..30u64).map(|i| if i < 29 { 0 } else { 8_200_000u64 }).collect::<Vec<_>>() }
            ]
        }),
    )
}

fn usage_lines(accounts: &[Value], used: &Value) -> Vec<String> {
    let accounts: Vec<AccountRow> = accounts.iter().map(from_json).collect();
    usage_table_lines(&accounts, &from_json::<UsedAnswer>(used), NOW)
}

/// `lines.slice(lines.indexOf("") + 1)`: what follows the blank line between the two tables.
fn after_blank(lines: &[String]) -> &[String] {
    &lines[lines.iter().position(|l| l.is_empty()).unwrap() + 1..]
}

mod wsp_usage_s_accounts_table {
    use super::*;

    #[test]
    fn says_each_account_s_banked_resets_and_when_the_first_lapses_and_nothing_for_an_a() {
        let codex = json!({ "key": "codex:acct_1", "agent": "codex", "label": "Codex with ChatGPT Plus", "computers": ["spoo"], "credits": { "count": 2, "nextExpiresAt": NOW + (21 * DAY) as f64, "readAt": NOW } });
        let claude = json!({ "key": "claude:vault-token", "agent": "claude", "label": "Claude Code with your sign-in", "computers": ["spoo"] });
        let lines = usage_lines(&[claude, codex], &empty_used());
        let (head, rows) = lines.split_first().unwrap();
        assert!(Regex::new(r"\bRESETS\b").unwrap().is_match(head), "{head}");
        let at = head.find("RESETS").unwrap();
        assert!(slice(&rows[1], at, None).starts_with("2 banked, first expires in 21 d"), "{rows:?}");
        assert_eq!(slice(&rows[0], at, Some(at + "RESETS".len())).trim(), "", "{rows:?}");
    }
}

mod wsp_usage_s_used_table {
    use super::*;

    #[test]
    fn names_the_agent_beside_each_model_since_two_agents_running_one_model_are_two_row() {
        let tokens = json!({ "input": 1_000, "output": 10, "cached": 0 });
        let by_model = with(
            empty_used(),
            json!({
                "split": "model",
                "rows": [
                    { "key": "claude:claude-sonnet-4-5", "label": "Sonnet 4.5", "agent": "claude", "tokens": tokens, "priced": false },
                    { "key": "hermes:claude-sonnet-4-5", "label": "Sonnet 4.5", "agent": "hermes", "tokens": tokens, "priced": false }
                ]
            }),
        );
        let lines = usage_lines(&[], &by_model);
        let [head, claude, hermes] = after_blank(&lines) else { panic!("{lines:?}") };
        assert!(Regex::new(r"^AGENT +MODEL +FRESH IN").unwrap().is_match(head), "{head}");
        assert!(Regex::new(r"^Claude Code +Sonnet 4\.5 ").unwrap().is_match(claude), "{claude}");
        assert!(Regex::new(r"^Hermes Agent +Sonnet 4\.5 ").unwrap().is_match(hermes), "{hermes}");
    }
}

mod wsp_usage_by_source {
    use super::*;

    #[test]
    fn sets_wsp_s_threads_beside_the_use_outside_wsp_each_with_its_fresh_written_cached() {
        let lines = usage_lines(&[], &used_by_source());
        let rows: Vec<Vec<String>> = after_blank(&lines).iter().map(|l| Regex::new(r" {2,}").unwrap().split(l).map(str::to_owned).collect()).collect();
        assert_eq!(
            rows,
            vec![
                strs(&["SOURCE", "FRESH IN", "WRITTEN", "CACHED", "OUT", "PRICE"]),
                strs(&["Outside wsp", "21.5M", "3.22B", "62.4B", "125M", "$44,925.05 list price"]),
                strs(&["wsp threads", "160k", "640k", "12.9M", "100k", "$8.63 list price"]),
            ]
        );
    }
}

mod wsp_usage_by_any_other_split {
    use super::*;

    #[tokio::test]
    async fn asks_for_what_was_used_as_the_usage_page_does_the_work_logged_outside_wsp_counte() {
        let host = ScriptedHost::replying(&[("usage.accounts", json!({ "ok": true, "accounts": [] })), ("usage.used", json!({ "ok": true, "used": used_by_account() }))]);
        let client = host.client(None).await.unwrap();
        let read = read_usage(client.as_ref(), UsageRange::Month, UsageSplit::Account).await.unwrap();
        // Both reads go out, the accounts asking for their limits now, then what was used, outside wsp counted.
        assert_eq!(
            host.asked(),
            vec![
                json!({ "fresh": true, "op": "usage.accounts" }),
                json!({ "range": "month", "split": "account", "outside": true, "op": "usage.used" }),
            ]
        );
        // Each answer as the host wrote it.
        assert_eq!(read, UsageRead { accounts: json!([]), used: used_by_account() });
        assert_eq!(serde_json::to_value(wsp_words::usage::used_ask(&UsageRange::Month, &UsageSplit::Account)).unwrap(), json!({ "range": "month", "split": "account", "outside": true }));
    }

    #[test]
    fn prints_each_account_with_the_use_outside_wsp_counted_in_as_the_page_draws_it() {
        let lines = usage_lines(&[], &used_by_account());
        let rows: Vec<Vec<String>> = after_blank(&lines).iter().map(|l| Regex::new(r" {2,}").unwrap().split(l).map(str::to_owned).collect()).collect();
        assert_eq!(
            rows,
            vec![
                strs(&["ACCOUNT", "FRESH IN", "WRITTEN", "CACHED", "OUT", "PRICE"]),
                strs(&["Claude Code with an API key", "21.6M", "3.22B", "62.4B", "125M", "$44,928.61 list price"]),
                strs(&["Codex with ChatGPT Plus", "350k", "0", "7.8M", "50k", "$7.15 list price"]),
                strs(&["Claude Code signed in on hetzner", "0", "0", "0", "0", "$0.00"]),
            ]
        );
    }
}

// ---------------------------------------------------------------------------------------------------------------
// The whole lines the TS printed, read off a run of the real functions. The TS tests above slice columns; these hold
// every character of a table, and the lines the TS tests never check (the empty listings, the saved and removed
// lines, the two tables joined by a blank line).
// ---------------------------------------------------------------------------------------------------------------

mod whole_lines_as_the_ts_printed_them {
    use super::*;

    #[test]
    fn place_lines_prints_the_three_places_whole() {
        assert_eq!(
            place_lines(&views(&[here(), box_row(), solari()])),
            strs(&[
                "PLACE        KIND      CORES  MEMORY  DISK FREE  COPIES  PRESENT    FORKS  LAST SEEN                 DEFAULT  BEHIND  IMAGE",
                "zingzys-mac  computer  8      16 GB                      yes",
                "box          computer  4      4 GB    831 GB             yes               2026-09-12T00:00:00.000Z  default",
                "solari       provider                                    $0.018/hr",
            ])
        );
    }

    #[test]
    fn place_lines_and_computer_lines_say_how_to_get_a_row_when_there_is_none() {
        assert_eq!(place_lines(&[]), strs(&["This host holds no place. wsp add prints the join line for a computer you are sitting at."]));
        assert_eq!(computer_lines(&[], "darwin", &[], &[]), strs(&["This host holds no computer. wsp add prints the join line for a computer you are sitting at."]));
    }

    #[test]
    fn computer_lines_prints_this_mac_a_box_and_a_cloud_whole() {
        assert_eq!(
            computer_lines(&views(&[here(), box_row(), solari()]), "darwin", &[], &[]),
            strs(&[
                "COMPUTER     KIND      CORES  MEMORY  DISK FREE  COPIES  PRESENT    WORKSPACES  THREADS  MACHINES  SPEND  STATE  LAST SEEN                 BEHIND  IMAGE  TOOLS  AGENTS",
                "zingzys-mac  this Mac  8      16 GB                      yes                                              Ready",
                "box          box       4      4 GB    831 GB             yes                                              Ready  2026-09-12T00:00:00.000Z",
                "Solari       cloud                                       $0.018/hr                                        Ready",
            ])
        );
    }

    #[test]
    fn computer_lines_names_the_provider_whose_id_is_box_by_its_app_name() {
        assert_eq!(
            computer_lines(&views(&[box_row(), boat(), solari()]), "darwin", &[], &[]),
            strs(&[
                "COMPUTER  KIND   CORES  MEMORY  DISK FREE  COPIES  PRESENT    WORKSPACES  THREADS  MACHINES  SPEND  STATE  LAST SEEN                 BEHIND  IMAGE  TOOLS  AGENTS",
                "box       box    4      4 GB    831 GB             yes                                              Ready  2026-09-12T00:00:00.000Z",
                "Boat      cloud                                    $0.16/hr                                         Ready",
                "Solari    cloud                                    $0.018/hr                                        Ready",
            ])
        );
    }

    #[test]
    fn recipe_lines_says_how_to_save_one_when_none_is_saved() {
        assert_eq!(
            recipe_lines(&[]),
            strs(&["No recipe is saved yet. The app's Add a computer saves one, and wsp recipes save <name> --from <computer> saves what a computer was set up with."])
        );
    }

    #[test]
    fn recipe_lines_prints_the_day_saved_and_the_computers_that_follow_each_recipe() {
        let saved = recipe(builders_file(), &["spoo", "vps"], Some("2026-10-05T12:00:00.000Z"));
        let unsaved = from_json::<RecipeView>(&json!({ "name": "laptop", "slug": "laptop", "summary": "2 CLIs", "machines": [], "file": { "name": "laptop" } }));
        assert_eq!(
            recipe_lines(&[saved, unsaved]),
            strs(&["RECIPE    HOLDS         SAVED       COMPUTERS", "Builders  1 MCP server  2026-10-05  spoo, vps", "laptop    2 CLIs"])
        );
    }

    #[test]
    fn recipe_shown_lines_prints_the_name_every_kind_who_follows_it_and_the_hash() {
        assert_eq!(
            recipe_shown_lines(&recipe(builders_file(), &[], None), "h"),
            strs(&["Builders: 1 MCP server", "  mcp      sentry (agents codex)", "  followed by no computer", "  hash     h"])
        );
        assert_eq!(
            recipe_shown_lines(&recipe(with(builders_file(), json!({ "copyKeys": true })), &["spoo"], None), "h"),
            strs(&["Builders: 1 MCP server", "  mcp      sentry (agents codex)", "  keys     copied with the servers that carry them", "  followed by spoo", "  hash     h"])
        );
    }

    #[test]
    fn recipe_saved_line_names_the_computers_that_follow_it_now() {
        assert_eq!(recipe_saved_line(&recipe(builders_file(), &[], None)), "saved Builders (1 MCP server)");
        assert_eq!(recipe_saved_line(&recipe(builders_file(), &["spoo", "vps"], None)), "saved Builders (1 MCP server), followed by spoo, vps");
    }

    #[test]
    fn recipe_removed_line_names_the_computers_that_follow_none_now() {
        assert_eq!(recipe_removed_line(&recipe(builders_file(), &[], None)), "removed Builders");
        assert_eq!(
            recipe_removed_line(&recipe(builders_file(), &["spoo", "vps"], None)),
            "removed Builders; spoo, vps keep what it put there and follow no recipe"
        );
    }

    #[test]
    fn usage_table_lines_prints_the_accounts_a_blank_line_and_what_was_used() {
        let codex = json!({ "key": "codex:acct_1", "agent": "codex", "label": "Codex with ChatGPT Plus", "computers": ["spoo"], "credits": { "count": 2, "nextExpiresAt": NOW + (21 * DAY) as f64, "readAt": NOW } });
        let claude = json!({ "key": "claude:vault-token", "agent": "claude", "label": "Claude Code with your sign-in", "computers": ["spoo"] });
        assert_eq!(
            usage_lines(&[claude, codex], &empty_used()),
            strs(&[
                "AGENT        ACCOUNT                        COMPUTERS  SESSION  WEEK  RESETS                           PLAN  STATE",
                "Claude Code  Claude Code with your sign-in  spoo",
                "Codex        Codex with ChatGPT Plus        spoo                      2 banked, first expires in 21 d",
                "",
                "Nothing used in this range.",
            ])
        );
        assert_eq!(usage_lines(&[], &empty_used()), strs(&["No agent is signed in on any computer.", "", "Nothing used in this range."]));
    }

    #[test]
    fn usage_table_lines_prints_the_models_with_the_agent_first_and_not_priced_where_it_is_not() {
        let tokens = json!({ "input": 1_000, "output": 10, "cached": 0 });
        let by_model = with(
            empty_used(),
            json!({
                "split": "model",
                "rows": [
                    { "key": "claude:claude-sonnet-4-5", "label": "Sonnet 4.5", "agent": "claude", "tokens": tokens, "priced": false },
                    { "key": "hermes:claude-sonnet-4-5", "label": "Sonnet 4.5", "agent": "hermes", "tokens": tokens, "priced": false }
                ]
            }),
        );
        assert_eq!(
            usage_lines(&[], &by_model),
            strs(&[
                "No agent is signed in on any computer.",
                "",
                "AGENT         MODEL       FRESH IN  WRITTEN  CACHED  OUT  PRICE",
                "Claude Code   Sonnet 4.5  1k        0        0       10   not priced",
                "Hermes Agent  Sonnet 4.5  1k        0        0       10   not priced",
            ])
        );
    }

    #[test]
    fn usage_table_lines_prints_the_two_splits_the_page_draws_whole() {
        assert_eq!(
            usage_lines(&[], &used_by_source()),
            strs(&[
                "No agent is signed in on any computer.",
                "",
                "SOURCE       FRESH IN  WRITTEN  CACHED  OUT   PRICE",
                "Outside wsp  21.5M     3.22B    62.4B   125M  $44,925.05 list price",
                "wsp threads  160k      640k     12.9M   100k  $8.63 list price",
            ])
        );
        assert_eq!(
            usage_lines(&[], &used_by_account()),
            strs(&[
                "No agent is signed in on any computer.",
                "",
                "ACCOUNT                           FRESH IN  WRITTEN  CACHED  OUT   PRICE",
                "Claude Code with an API key       21.6M     3.22B    62.4B   125M  $44,928.61 list price",
                "Codex with ChatGPT Plus           350k      0        7.8M    50k   $7.15 list price",
                "Claude Code signed in on hetzner  0         0        0       0     $0.00",
            ])
        );
    }
}
