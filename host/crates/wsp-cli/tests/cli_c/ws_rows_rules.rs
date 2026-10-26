// SPDX-License-Identifier: AGPL-3.0-only
//! Hand-written rules of the `ws_rows` unit (`spec/ws-rows.md`). The recorded cases pin the TS's output; these pin the
//! laws it keeps for inputs a recording cannot list.

use serde_json::{json, Value};
use wsp_cli::verbs::workspaces_help::reads::ThreadRow;
use wsp_cli::verbs::workspaces_help::rows as w;
use wsp_wire::views::workspace_more::WorkspaceOut;
use wsp_words::workspace_state::StandsOn;

use super::common::{case, holds, Case};
use super::drivers_ws_rows as d;

const UNIT: &str = "ws_rows";

fn recorded(func: &str, name: &str) -> Case {
    case(UNIT, func, name, "<cwd>")
}

fn workspace(kind: &str, machine: &str, worktree: Option<Value>) -> WorkspaceOut {
    let mut v = json!({
        "id": "ws-1", "name": "alpha", "machineId": machine, "phase": "running", "kind": kind, "golden": "snap-g1",
        "createdAt": "2026-09-27T01:02:03.004Z",
        "project": { "id": "proj-1", "name": "alpha", "path": "/root/alpha", "computer": "place-9" }, "home": "/root",
    });
    if let Some(wt) = worktree {
        v["worktree"] = wt;
    }
    serde_json::from_value(v).expect("a workspace reads")
}

fn row(id: &str, parent: Option<&str>) -> ThreadRow {
    let mut v = json!({
        "id": id, "workspaceId": "ws-1", "harness": "claude", "startedBy": "person", "status": "completed", "title": "t",
        "sessionId": "s-1", "turns": 1, "ran": true, "projectName": "wsp", "folder": "/root/wsp", "branch": "main", "computerName": "attic",
    });
    if let Some(p) = parent {
        v["parentThreadId"] = json!(p);
    }
    serde_json::from_value(v).expect("a thread row reads")
}

/// Invariant 7 (packages/host/AGENTS.md): with the cloud off no line names a cloud. A workspace of the kinds that exist
/// with the cloud off (local and place) says "computer" on all four of its lines, whatever it stands on or holds.
#[test]
fn invariant_7_the_lines_of_local_and_place_workspaces_name_no_cloud() {
    let wt = json!({ "path": "/Users/me/.wsp/worktrees/proj-1/work", "branch": "work", "made": true });
    for (kind, machine) in [("local", "here"), ("place", "m-1")] {
        for worktree in [None, Some(wt.clone())] {
            for on in [None, Some(StandsOn { name: "alpha".into(), computer: "attic".into() })] {
                for threads in [0usize, 1, 3] {
                    let d = w::Dropping { workspace: workspace(kind, machine, worktree.clone()), threads, on: on.clone() };
                    for line in [w::forget_question(&d), w::forgot_line(&d), w::delete_question(&d), w::deleted_line(&d)] {
                        assert!(!line.to_lowercase().contains("cloud"), "{kind} {line:?}");
                    }
                }
            }
        }
    }
    for fixed in [w::NO_PROJECT_YET.to_string(), w::THREAD_HEAD.join(" ")] {
        assert!(!fixed.to_lowercase().contains("cloud"), "{fixed:?}");
    }
}

/// A listing's rows each print once, a parent before the rows under it, one step deeper than it, and the rows with no
/// parent in the listing at depth 0, in listing order. Shapes are made by a fixed counter so a run is the same each time.
#[test]
fn thread_tree_prints_each_row_once_with_parents_first() {
    let mut seed: u64 = 0x2449;
    let mut next = move |n: u64| {
        seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        (seed >> 33) % n
    };
    for round in 0..200 {
        let size = 1 + next(12) as usize;
        let ids: Vec<String> = (0..size).map(|i| format!("t{i}")).collect();
        let rows: Vec<ThreadRow> = (0..size)
            .map(|i| {
                // A parent that is later in the listing, earlier, absent or none; never itself, so no row is in a loop.
                let parent = match next(4) {
                    0 => None,
                    1 => Some("absent".to_string()),
                    _ => {
                        let at = next(size as u64) as usize;
                        (at != i).then(|| ids[at].clone())
                    }
                };
                row(&ids[i], parent.as_deref())
            })
            .collect();
        let held: Vec<&str> = ids.iter().map(String::as_str).collect();
        let tree = w::thread_tree(&rows);
        // Loops of parents are possible among the generated rows; those rows stand flat at depth 0 and still print once.
        assert_eq!(tree.len(), rows.len(), "round {round}: every row prints once");
        let mut seen = std::collections::HashSet::new();
        for n in &tree {
            assert!(seen.insert(n.row.thread.id.clone()), "round {round}: {} printed twice", n.row.thread.id);
        }
        for (at, n) in tree.iter().enumerate() {
            let parent = n.row.thread.parent_thread_id.as_deref().filter(|p| held.contains(p));
            let Some(parent) = parent else {
                assert_eq!(n.depth, 0, "round {round}: a row with no parent here is a top row");
                continue;
            };
            if n.depth == 0 {
                continue;
            }
            let above = tree[..at].iter().rposition(|m| m.depth + 1 == n.depth).expect("a row one step up");
            assert_eq!(tree[above].row.thread.id, parent, "round {round}: {} sits under its parent", n.row.thread.id);
        }
    }
}

/// A cell never holds more UTF-16 units than its width, for text whose characters are one unit each.
#[test]
fn shortened_cells_never_exceed_their_width() {
    let units = |s: &str| s.encode_utf16().count();
    for width in 1..=70usize {
        for n in [0usize, width.saturating_sub(1), width, width + 1, width + 30] {
            let text: String = "aé/b".chars().cycle().take(n).collect();
            for shortened in [w::shortened_front(&text, width), w::shortened_end(&text, width)] {
                assert!(units(&shortened) <= width.max(1), "width {width}, {n} units: {shortened:?}");
            }
            if units(&text) <= width {
                assert_eq!(w::shortened_front(&text, width), text);
                assert_eq!(w::shortened_end(&text, width), text);
            } else {
                assert!(w::shortened_front(&text, width).starts_with('…'));
                assert!(w::shortened_end(&text, width).ends_with('…'));
            }
        }
    }
}

/// A thread's own row is followed by one row per subagent, in order, each repeating the thread's project, folder, branch
/// and computer cells, one step in from the thread's id, with the thread's agent.
#[test]
fn thread_lines_put_one_row_per_subagent_under_the_thread() {
    let mut v = serde_json::to_value(row("t-1", None)).expect("a row serializes");
    v["subagents"] = json!([
        { "id": "a1", "title": "count", "state": "done", "startedAt": 1727431200000u64 },
        { "id": "b2", "title": "read", "state": "failed", "startedAt": 1727431200000u64 },
    ]);
    let t: ThreadRow = serde_json::from_value(v).expect("a row with subagents reads");
    let lines = w::thread_lines(&t, "", Some("/home/zed"), 0.0);
    assert_eq!(lines.len(), 3);
    for sub in &lines[1..] {
        assert_eq!(sub.len(), w::THREAD_HEAD.len());
        for at in [0usize, 1, 2, 5, 8] {
            assert_eq!(sub[at], lines[0][at], "column {}", w::THREAD_HEAD[at]);
        }
        assert_eq!(sub[3], "  t-1");
        assert_eq!(sub[7], "agent");
    }
    assert_eq!([lines[1][4].as_str(), lines[2][4].as_str()], ["a1", "b2"]);
    assert_eq!(lines[0][4], "");
}

/// verbs-lifecycle.test.ts: "the delete question and its line say what the delete does to this kind's machine".
#[test]
fn ts_verbs_lifecycle_the_delete_question_and_its_line_say_what_the_delete_does_to_the_machine() {
    let table: [(&str, &str, &str); 3] = [
        ("lifecycle test: this computer's own", "Delete box?\nIts computer is left as it is; its record and 1 thread leave this computer.", "deleted box ws_mine: its computer is left as it is, and its record and 1 thread are gone from this computer"),
        ("lifecycle test: a worktree wsp made", "Delete box?\nIts worktree at /Users/dev/api-fix is removed and the project folder is left as it is; its record and 1 thread leave this computer.", "deleted box ws_mine: its worktree at /Users/dev/api-fix is removed and the project folder is left as it is, and its record and 1 thread are gone from this computer"),
        ("lifecycle test: a fork", "Delete box?\nIts computer is deleted in the cloud; its record and 0 threads leave this computer.", "deleted box ws_mine: computer m_ab12 is gone in the cloud, and its record and 0 threads are gone from this computer"),
    ];
    for (name, question, line) in table {
        assert_eq!(recorded("deleteQuestion", name).value.unwrap(), question, "{name}");
        assert_eq!(recorded("deletedLine", name).value.unwrap(), line, "{name}");
        holds(UNIT, "deleteQuestion", name, d::delete_question);
        holds(UNIT, "deletedLine", name, d::deleted_line);
    }
}

/// verbs-lifecycle.test.ts: "a copy on a computer somebody joined is deleted from that computer by the names a person
/// knows, never the cloud or the machine's id": by its name where the places read, by `that computer` where they do not,
/// and a provider's fork takes the cloud's words.
#[test]
fn ts_verbs_lifecycle_a_copy_on_a_joined_computer_is_named_by_the_names_a_person_knows() {
    let named = recorded("deleting", "a copy on a computer somebody joined is named by the computer's name");
    assert_eq!(named.value.as_ref().unwrap()["on"], serde_json::json!({ "name": "fix-login", "computer": "spoo" }));
    let unread = recorded("deleting", "a caller that may not read the computers' names still never reads the machine's id");
    assert_eq!(unread.value.as_ref().unwrap()["on"], serde_json::json!({ "name": "fix-login", "computer": "that computer" }));
    let cloud = recorded("deleting", "a fork at another provider's account takes the cloud's words");
    assert!(cloud.value.as_ref().unwrap().get("on").is_none());
    for name in [&named.case, &unread.case, &cloud.case] {
        holds(UNIT, "deleting", name, d::deleting);
    }
    assert_eq!(
        recorded("deleteQuestion", "lifecycle test: a copy on spoo").value.unwrap(),
        "Delete fix-login?\nIts copy on spoo is deleted; its record and 0 threads leave this computer."
    );
    assert_eq!(
        recorded("deletedLine", "lifecycle test: a copy on spoo").value.unwrap(),
        "deleted fix-login ws_fix: fix-login is deleted from spoo, and its record and 0 threads are gone from this computer"
    );
    assert_eq!(
        recorded("deleteQuestion", "lifecycle test: a copy on that computer").value.unwrap(),
        "Delete fix-login?\nIts copy on that computer is deleted; its record and 0 threads leave this computer."
    );
    for name in ["lifecycle test: a copy on spoo", "lifecycle test: a copy on that computer"] {
        holds(UNIT, "deleteQuestion", name, d::delete_question);
        holds(UNIT, "deletedLine", name, d::deleted_line);
    }
}

/// verbs-folders-settings.test.ts: two rows naming each other are under no top row, and the listing prints every row it
/// was given all the same.
#[test]
fn ts_verbs_folders_settings_two_rows_naming_each_other_are_both_printed() {
    let name = "two rows that are each other's parent stand flat at the top";
    let got: Vec<String> =
        recorded("threadTree", name).value.unwrap().as_array().unwrap().iter().map(|n| n["id"].as_str().unwrap().to_string()).collect();
    assert_eq!(got, ["a", "b"]);
    holds(UNIT, "threadTree", name, d::thread_tree);
}

/// verbs-threads.test.ts: "a sub-thread's state cell says what its machine is doing where that says more than the turn",
/// and "a thread whose machine naps past the keep of its disk reads Expired ...": the STATE cell (the seventh) of each case.
#[test]
fn ts_verbs_threads_the_state_cell_of_a_sub_thread_and_of_an_expired_disk() {
    let table = [
        ("a sub-thread waiting for a slot", "Waiting"),
        ("a sub-thread starting", "Starting"),
        ("a sub-thread running with nothing held", "Working"),
        ("a sub-thread napping with work not pushed", "Not pushed"),
        ("a sub-thread napping with nothing held", "Idle"),
        ("a sub-thread released after a completed turn", "Released"),
        ("a sub-thread released after an interrupted turn", "Released"),
        ("a sub-thread released and unseen reads done", "Done"),
        ("a sub-thread released after a failed turn", "Failed"),
        ("a sub-thread with work not pushed that asks reads needs you", "Needs you"),
        ("a disk kept until long ago reads expired", "Expired"),
        ("a disk kept until the far future reads idle", "Idle"),
        ("an expired disk of a sub-thread wins over its own word", "Expired"),
        ("an expired disk with a question reads needs you", "Needs you"),
        ("an expired disk of a failed turn reads failed", "Failed"),
    ];
    for (name, word) in table {
        let c = recorded("threadLines", name);
        assert_eq!(c.value.as_ref().unwrap()[0][6], word, "{name}");
        holds(UNIT, "threadLines", name, d::thread_lines);
    }
}

/// places.test.ts: a cloud is named by the name the app gives it, Boat and Solari, never the id stored state holds.
#[test]
fn ts_places_place_names_name_a_cloud_by_the_name_the_app_gives_it() {
    let boat = recorded("placeNames", "the box provider by its key name");
    assert_eq!(boat.value.unwrap(), serde_json::json!([["p-b", "Boat"]]));
    let all = recorded("placeNames", "a computer, a provider and a joined box");
    assert_eq!(all.value.unwrap(), serde_json::json!([["here", "this mac"], ["place-9", "attic"], ["place-s", "Solari"]]));
    holds(UNIT, "placeNames", "the box provider by its key name", d::place_names);
    holds(UNIT, "placeNames", "a computer, a provider and a joined box", d::place_names);
}

/// A size asked of a copy of a folder, and a size the provider does not offer, are refused before a stage is streamed or a
/// machine is asked for: the only frames a refused size leaves behind are the landing read, or none at all.
#[test]
fn a_refused_size_leaves_no_subscription_and_no_create() {
    let on_copy = recorded("createFor", "a size on a copy of a folder is refused before anything is sent");
    assert!(on_copy.frames.is_empty());
    holds(UNIT, "createFor", "a size on a copy of a folder is refused before anything is sent", d::create_for);
    for name in
        ["a size the provider does not offer", "a size that is no size", "an empty size word", "a size when the provider offers none"]
    {
        let c = recorded("createFor", name);
        assert_eq!(c.frames.len(), 1, "{name}");
        assert!(c.frames[0].contains("\"op\":\"workspaces.landing\""), "{name}");
        assert!(c.streamed.is_none() && c.stdout.is_empty(), "{name}");
        holds(UNIT, "createFor", name, d::create_for);
    }
}
