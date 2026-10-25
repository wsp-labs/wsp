// SPDX-License-Identifier: AGPL-3.0-only
//! Ported from packages/protocol/test/daemon-contract.test.ts:268-325: the daemon's contract fixtures, one accept file
//! and one reject file per op and per event, which the daemon's own Rust tests read too. The host's wire must take
//! every accept frame and refuse every reject frame, under the reader the TS test picks for the name: daemon frames.
mod common;
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use serde_json::Value;
use wsp_wire::wire::daemon::{parse_daemon_request, DaemonAuthRequest, PipeOpen, DAEMON_REQUEST_OPS};
use wsp_wire::wire::daemon_events::{parse_daemon_event, DAEMON_EVENT_TYPES};
use wsp_wire::wire::machine_link::{parse_machine_link_request, MACHINE_LINK_OPS};
use wsp_wire::wire::runtime::{parse_runtime_request, RuntimeOp};
use wsp_wire::wire::{parse, SchemaError};

fn contract() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../daemon/fixtures/contract")
}

/// The frames a file holds, which is always a non-empty array.
fn frames(dir: &str, name: &str) -> Vec<Value> {
    let path = contract().join(dir).join(name);
    let held: Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    let list = held.as_array().unwrap_or_else(|| panic!("{} must hold an array", path.display())).clone();
    assert!(!list.is_empty(), "{} must hold a non-empty array of frames", path.display());
    list
}

/// The op or type names with a file pair under a folder, off the files.
fn filed(dir: &str) -> BTreeSet<String> {
    std::fs::read_dir(contract().join(dir))
        .unwrap()
        .map(|e| e.unwrap().file_name().into_string().unwrap())
        .filter_map(|f| f.strip_suffix(".accept.json").or_else(|| f.strip_suffix(".reject.json")).map(str::to_owned))
        .collect()
}

/// The TS test's schemaFor: auth, the two frames a place sends its host and the one a pipe opens with each have their
/// own reader; a machine op is the link's union; every other op is DaemonRequest's.
fn read_frame(op: &str, frame: &Value) -> Result<(), SchemaError> {
    match op {
        "auth" => parse::<DaemonAuthRequest>(frame).map(|_| ()),
        "pipe.open" => parse::<PipeOpen>(frame).map(|_| ()),
        "place.auth" => parse_runtime_request(frame).map(|r| assert!(matches!(r.op, RuntimeOp::PlaceAuth(_)), "{frame}")),
        "place.prove" => parse_runtime_request(frame).map(|r| assert!(matches!(r.op, RuntimeOp::PlaceProve(_)), "{frame}")),
        machine if machine.starts_with("machine.") => parse_machine_link_request(frame).map(|_| ()),
        _ => parse_daemon_request(frame).map(|_| ()),
    }
}

// daemon-contract.test.ts:268 and 272, with every pair present (the case after them). The name lists are constants,
// so the case reads the first accept file through its reader first.
#[test]
fn names_one_file_pair_per_op_and_per_event_type_and_both_halves_of_each() {
    let first = &frames("frames", "ping.accept.json")[0];
    read_frame("ping", first).unwrap();
    let mut want: BTreeSet<String> = DAEMON_REQUEST_OPS.iter().chain(MACHINE_LINK_OPS).map(|s| s.to_string()).collect();
    want.extend(["auth", "place.auth", "place.prove", "pipe.open"].map(String::from));
    assert_eq!(filed("frames"), want);
    assert_eq!(filed("events"), DAEMON_EVENT_TYPES.iter().map(|s| s.to_string()).collect::<BTreeSet<_>>());
    for dir in ["frames", "events"] {
        for name in filed(dir) {
            for half in ["accept", "reject"] {
                assert!(contract().join(dir).join(format!("{name}.{half}.json")).exists(), "{dir}/{name}.{half}.json");
            }
        }
    }
}

// daemon-contract.test.ts:292-306: every frame under an op's accept file is taken and is that op's; every frame under
// its reject file is refused.
#[test]
fn the_wire_takes_every_accept_frame_and_refuses_every_reject_frame() {
    let mut failures = Vec::new();
    for op in filed("frames") {
        for frame in frames("frames", &format!("{op}.accept.json")) {
            if let Err(e) = read_frame(&op, &frame) {
                failures.push(format!("{op} must accept {frame}: {e:?}"));
            }
            assert_eq!(frame["op"], Value::String(op.clone()), "a frame under {op}'s file is {op}'s");
        }
        for frame in frames("frames", &format!("{op}.reject.json")) {
            if read_frame(&op, &frame).is_ok() {
                failures.push(format!("{op} must refuse {frame}"));
            }
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

// daemon-contract.test.ts:308-322
#[test]
fn the_wire_takes_every_accept_event_and_refuses_every_reject_event() {
    let mut failures = Vec::new();
    for kind in filed("events") {
        for event in frames("events", &format!("{kind}.accept.json")) {
            match parse_daemon_event(&event) {
                Ok(read) => assert_eq!(serde_json::to_value(&read).unwrap()["type"], Value::String(kind.clone()), "{event}"),
                Err(e) => failures.push(format!("{kind} must accept {event}: {e:?}")),
            }
        }
        for event in frames("events", &format!("{kind}.reject.json")) {
            if parse_daemon_event(&event).is_ok() {
                failures.push(format!("{kind} must refuse {event}"));
            }
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

// An accept event read and written back is the frame it was: the host relays a daemon's events to a page untouched
// in meaning, so a field the reader dropped or a key it renamed would reach the page changed.
#[test]
fn an_accept_event_writes_back_as_the_value_it_was() {
    for kind in filed("events") {
        for event in frames("events", &format!("{kind}.accept.json")) {
            let read = parse_daemon_event(&event).unwrap_or_else(|e| panic!("{kind}: {e:?}"));
            assert_eq!(serde_json::to_value(&read).unwrap(), event, "{kind}");
        }
    }
}
