// SPDX-License-Identifier: AGPL-3.0-only
//! Ported from packages/protocol/test/places.test.ts (the one word a row says about the daemon a place runs),
//! broker-contract.test.ts and protocol.test.ts, with the edges written here against what the TS module printed:
//! daemon_version.
mod common;
use common::*;
use serde_json::json;
use wsp_wire::wire::daemon_version::*;

// places.test.ts:399
#[test]
fn names_both_versions_when_this_wsp_deploys_a_newer_daemon_than_the_computer_runs() {
    assert_eq!(place_daemon_behind(Some(27)), Some(format!("daemon 27, host {DAEMON_VERSION}")));
    assert_eq!(place_daemon_behind(Some(DAEMON_VERSION - 1)), Some(format!("daemon {}, host {DAEMON_VERSION}", DAEMON_VERSION - 1)));
    // A daemon that reported 0 is behind, not unknown.
    assert_eq!(place_daemon_behind(Some(0)), Some(format!("daemon 0, host {DAEMON_VERSION}")));
}

// places.test.ts:404
#[test]
fn says_nothing_of_a_computer_that_is_level_one_that_is_ahead_or_one_that_never_reported() {
    assert_eq!(place_daemon_behind(Some(DAEMON_VERSION)), None);
    // A computer running a daemon from a newer host than this one is not behind, and a row that said so would send
    // a person to move it backwards.
    assert_eq!(place_daemon_behind(Some(DAEMON_VERSION + 1)), None);
    assert_eq!(place_daemon_behind(None), None);
}

// places.test.ts:412
#[test]
fn answers_the_word_with_the_line_that_moves_it_which_is_the_flag_on_the_verb_that_joins() {
    assert_eq!(place_update_line("spoo"), "wsp add spoo --update");
    let word = place_daemon_behind(Some(27)).unwrap();
    assert_eq!(
        place_behind_line("spoo", &word),
        format!("spoo is behind: daemon 27, host {DAEMON_VERSION}; wsp add spoo --update puts this wsp's daemon on it")
    );
}

// places.test.ts:435 (the placeNoChipLine half is wsp-words')
#[test]
fn refuses_a_place_already_on_this_daemon_naming_what_it_read() {
    assert_eq!(
        place_current_line("spoo", DAEMON_VERSION),
        format!("spoo already runs daemon {DAEMON_VERSION}, which is the one this wsp deploys")
    );
}

// broker-contract.test.ts:69, and every gate the module names: each is a daemon this tree ships or one before it.
#[test]
fn every_gate_is_a_daemon_this_tree_ships_or_one_before_it() {
    assert_eq!(DAEMON_VERSION, wsp_frames::numbers::DAEMON_VERSION as i64);
    for gate in [
        STDIO_DAEMON_VERSION,
        HTTP_POST_DAEMON_VERSION,
        PIPE_DAEMON_VERSION,
        NAMED_WATCH_DAEMON_VERSION,
        SYS_UNWATCH_DAEMON_VERSION,
        FS_FOLDERS_DAEMON_VERSION,
        FORK_BRANCH_DAEMON_VERSION,
        LEAVE_ASKS_DAEMON_VERSION,
        LEAVE_TAKES_DAEMON_VERSION,
    ] {
        assert!(gate <= DAEMON_VERSION, "{gate}");
    }
    // Paired with a call through the module, so the case is red until the module is.
    assert_eq!(
        place_daemon_behind(Some(PIPE_DAEMON_VERSION - 1)),
        Some(format!("daemon {}, host {DAEMON_VERSION}", PIPE_DAEMON_VERSION - 1))
    );
}

// protocol.test.ts:1311: the value, not the expression, since DAEMON_CONTENT_SHA hashes it.
#[test]
fn names_the_roots_file_of_the_guest_s_home() {
    assert_eq!(DAEMON_ROOTS_PATH, "/root/.wsp/roots");
    assert_eq!(DAEMON_ROOTS_PATH, wsp_frames::numbers::DAEMON_ROOTS_PATH);
    assert_eq!(DAEMON_CONTENT_SHA.len(), 64);
    assert!(DAEMON_CONTENT_SHA.bytes().all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase()));
    assert_eq!(place_daemon_behind(Some(1)), Some(format!("daemon 1, host {DAEMON_VERSION}")));
}

// daemon-version.ts:793, printed by the TS module.
#[test]
fn a_named_watch_needs_a_daemon_of_136_and_says_so_of_one_that_never_said_its_version() {
    assert_eq!(named_watch_gap(None), Some(format!("daemon version unknown, host {DAEMON_VERSION}")));
    assert_eq!(named_watch_gap(Some(0)), Some(format!("daemon 0, host {DAEMON_VERSION}")));
    assert_eq!(named_watch_gap(Some(NAMED_WATCH_DAEMON_VERSION - 1)), Some(format!("daemon 135, host {DAEMON_VERSION}")));
    assert_eq!(named_watch_gap(Some(NAMED_WATCH_DAEMON_VERSION)), None);
    assert_eq!(named_watch_gap(Some(DAEMON_VERSION)), None);
}

// daemon-version.ts:811 and 818: read off the wsp's own version, absent reading as 0.
#[test]
fn the_leave_a_computer_runs_asks_from_141_and_takes_from_145() {
    for (said, asks, takes) in [
        (None, false, false),
        (Some(0), false, false),
        (Some(140), false, false),
        (Some(141), true, false),
        (Some(144), true, false),
        (Some(145), true, true),
        (Some(DAEMON_VERSION), true, true),
    ] {
        assert_eq!((leave_asks(said), leave_takes(said)), (asks, takes), "{said:?}");
    }
}

// daemon-version.ts:826-835, printed by the TS module.
#[test]
fn the_kill_and_slate_lines_name_the_computer_and_the_update() {
    assert_eq!(
        kill_too_old_line("box"),
        "TERM went to this process alone: box runs a daemon too old to end what it started; wsp add box --update updates it"
    );
    let one = [KilledAlone { pid: 12.into(), command: "node x".into() }];
    assert_eq!(
        kill_too_old_of_line("box", &one),
        "TERM went to 12 (node x) alone: box runs a daemon too old to end what it started; wsp add box --update updates it"
    );
    let two = [KilledAlone { pid: 12.into(), command: "node x".into() }, KilledAlone { pid: 13.into(), command: "vite".into() }];
    assert_eq!(
        kill_too_old_of_line("box", &two),
        "TERM went to 12 (node x) and 13 (vite) alone: box runs a daemon too old to end what they started; wsp add box --update updates it"
    );
    // An empty list is no caller's, and the TS writes the sentence with nothing in it and "they".
    assert_eq!(
        kill_too_old_of_line("box", &[]),
        "TERM went to  alone: box runs a daemon too old to end what they started; wsp add box --update updates it"
    );
    assert_eq!(
        slate_server_behind_line("box"),
        "box runs a daemon too old to start a slate's MCP server there; wsp add box --update updates it"
    );
}

// daemon-version.ts:746-760: the record a save writes about the build that wrote it.
#[test]
fn a_state_shape_names_its_writer_by_binary_build_and_daemon() {
    let wrote: StateShape =
        reads_exact(json!({ "shape": 2, "wsp": "0.3.8", "daemon": 167, "bin": "/usr/local/bin/wsp", "at": "2026-10-25T00:00:00.000Z" }));
    assert_eq!(wrote.shape.get(), STATE_SHAPE);
    assert_eq!(state_writer_words(&wrote), "/usr/local/bin/wsp (wsp 0.3.8, daemon 167)");
    // shape and daemon are integers; every field is required.
    assert_eq!(refused::<StateShape>(json!({ "shape": 2.5, "wsp": "x", "daemon": 1, "bin": "b", "at": "t" })).path, path!["shape"]);
    assert_eq!(refused::<StateShape>(json!({ "shape": 2, "wsp": "x", "daemon": 1, "bin": "b" })).path, path!["at"]);
}
