// SPDX-License-Identifier: AGPL-3.0-only
//! The event bus: runtime/src/types/events.ts at 65f6b89c7, spec/events.md, chapter 03 rules 3.6, 3.7, 3.20 to 3.22
//! and 3.28. Expected values are the TS bus's own answers (tests/fixtures/core_types/events.json).
#![allow(unused_must_use)]
mod common;

use std::cell::RefCell;
use std::rc::Rc;

use common::{fixture, json_of, wire};
use serde_json::{json, Value};
use wsp_core::types::events::*;
use wsp_wire::wire::events_union::EventUnion;
use wsp_words::exit::HostError;

const STREAM: &str = "11111111-2222-4333-8444-555555555555";

fn renamed(n: u32) -> EventUnion {
    wire(&json!({ "type": "workspace.renamed", "workspaceId": format!("w{n}"), "name": format!("n{n}") }))
}

fn value(e: &EventUnion) -> Value {
    serde_json::to_value(e).expect("an event writes")
}

fn since_value(s: &Since) -> Value {
    json!({ "stream": s.stream, "head": s.head, "events": s.events.iter().map(value).collect::<Vec<_>>(), "gap": s.gap })
}

fn broke(message: &str) -> HostError {
    HostError {
        message: message.into(),
        fix: None,
        kind: None,
        errors: None,
        warnings: None,
        left: None,
        host_key: None,
        code: None,
        status: None,
        marks: vec![],
    }
}

type Log = Rc<RefCell<Vec<String>>>;

fn log() -> Log {
    Rc::new(RefCell::new(Vec::new()))
}

/// Records `<tag>:<workspaceId>:<seq or ->` for every event it hears.
fn recorder(log: &Log, tag: &'static str) -> EventListener {
    let log = log.clone();
    Rc::new(move |e: &EventUnion| {
        let v = value(e);
        let seq = v.get("seq").map(|s| s.to_string()).unwrap_or_else(|| "-".into());
        log.borrow_mut().push(format!("{tag}:{}:{seq}", v["workspaceId"].as_str().unwrap_or("?")));
        Ok(())
    })
}

// events.ts:74-75: no cursor is no replay and no gap, on a fresh bus too.
#[test]
fn since_with_no_cursor_on_a_fresh_bus_answers_head_zero_and_no_gap() {
    let f = fixture("events");
    let bus = event_bus(STREAM.into());
    assert_eq!(since_value(&bus.since(Some(0), None)), f["fresh"]);
    assert_eq!(since_value(&bus.since(None, None)), json!({ "stream": STREAM, "head": 0, "events": [], "gap": false }));
}

// events.ts:78: a cursor past the head is a gap, even on a bus that has emitted nothing.
#[test]
fn since_a_cursor_past_a_fresh_head_is_a_gap() {
    let f = fixture("events");
    let bus = event_bus(STREAM.into());
    assert_eq!(since_value(&bus.since(Some(1), None)), f["freshAfter1"]);
}

// events.ts:77: an empty stream string is still a stream, and not this one, so it is a gap.
#[test]
fn since_an_empty_stream_with_a_cursor_is_foreign() {
    let f = fixture("events");
    let bus = event_bus(STREAM.into());
    assert_eq!(since_value(&bus.since(Some(0), Some(""))), f["freshEmptyStream"]);
}

// events.ts:75: with no cursor the stream is never read, so a foreign one is no gap.
#[test]
fn since_with_no_cursor_never_reads_the_stream() {
    let f = fixture("events");
    let bus = event_bus(STREAM.into());
    assert_eq!(since_value(&bus.since(None, Some("other"))), f["freshNoAfterForeign"]);
}

fn emitted_bus() -> (Rc<Bus>, Log) {
    let bus = event_bus(STREAM.into());
    let seen = log();
    bus.on(ANY_EVENT, recorder(&seen, "any"));
    bus.on("workspace.renamed", recorder(&seen, "typed"));
    bus.pass(renamed(0)).unwrap();
    bus.emit(renamed(1)).unwrap();
    bus.emit(renamed(2)).unwrap();
    bus.pass(renamed(3)).unwrap();
    (bus, seen)
}

// events.ts:81-96: typed listeners hear an event before "*" ones whatever order they were added in; a pass before any
// emit carries no seq, one after carries the head's.
#[test]
fn delivery_runs_typed_listeners_before_any_and_pass_takes_the_head() {
    let (_bus, seen) = emitted_bus();
    assert_eq!(
        *seen.borrow(),
        vec!["typed:w0:-", "any:w0:-", "typed:w1:1", "any:w1:1", "typed:w2:2", "any:w2:2", "typed:w3:2", "any:w3:2"]
    );
}

// events.ts:82-84: emit stamps seq from 1 and rings the event; pass rings nothing.
#[test]
fn emit_rings_each_event_with_its_seq_and_pass_rings_nothing() {
    let f = fixture("events");
    let (bus, _) = emitted_bus();
    assert_eq!(since_value(&bus.since(Some(0), None)), f["after0"]);
}

// events.ts:79: a cursor at the head replays nothing and is no gap.
#[test]
fn since_at_the_head_replays_nothing_and_is_no_gap() {
    let f = fixture("events");
    let (bus, _) = emitted_bus();
    assert_eq!(since_value(&bus.since(Some(2), None)), f["after2"]);
}

// events.ts:78: one past the head is a gap.
#[test]
fn since_one_past_the_head_is_a_gap() {
    let f = fixture("events");
    let (bus, _) = emitted_bus();
    assert_eq!(since_value(&bus.since(Some(3), None)), f["after3"]);
}

// events.ts:77-79: this bus's own stream replays after the cursor; any other stream is a gap.
#[test]
fn since_reads_its_own_stream_and_refuses_another() {
    let f = fixture("events");
    let (bus, _) = emitted_bus();
    assert_eq!(since_value(&bus.since(Some(1), Some(STREAM))), f["after1Own"]);
    assert_eq!(since_value(&bus.since(Some(1), Some("x"))), f["after1Foreign"]);
}

// Rule 3.20, 3.24: seq is the event's last key, the same object in the ring as the listener heard.
#[test]
fn a_ringed_event_writes_seq_last() {
    let bus = event_bus(STREAM.into());
    let heard: Rc<RefCell<Vec<String>>> = log();
    let h = heard.clone();
    bus.on(
        ANY_EVENT,
        Rc::new(move |e: &EventUnion| {
            h.borrow_mut().push(json_of(e));
            Ok(())
        }),
    );
    assert_eq!(bus.emit(renamed(1)).unwrap(), 1);
    let ringed = bus.since(Some(0), None).events;
    assert_eq!(json_of(&ringed[0]), r#"{"type":"workspace.renamed","workspaceId":"w1","name":"n1","seq":1}"#);
    assert_eq!(*heard.borrow(), vec![json_of(&ringed[0])]);
}

// events.ts:84, EVENT_RING_CAP: the ring keeps the newest 5000; a cursor older than oldest - 1 is a gap.
#[test]
fn the_ring_keeps_the_newest_five_thousand() {
    let f = &fixture("events")["wrapped"];
    let bus = event_bus(STREAM.into());
    for i in 1..=5003 {
        assert_eq!(bus.emit(renamed(i)).unwrap(), u64::from(i));
    }
    let a = bus.since(Some(0), None);
    assert_eq!((a.gap, a.head, a.events.len()), (true, 5003, 0));
    assert_eq!(f["after0"], json!({ "gap": true, "head": 5003, "n": 0 }));
    let b = bus.since(Some(2), None);
    assert_eq!((b.gap, b.events.len()), (true, 0));
    let c = bus.since(Some(3), None);
    assert!(!c.gap);
    assert_eq!(c.events.len(), 5000);
    assert_eq!(value(&c.events[0]), f["after3"]["first"]);
    assert_eq!(value(c.events.last().unwrap()), f["after3"]["last"]);
    assert_eq!(bus.state.borrow().ring.len(), EVENT_RING_CAP);
    let d = bus.since(Some(5003), None);
    assert_eq!((d.gap, d.events.len()), (false, 0));
    let e = bus.since(Some(5002), None);
    assert_eq!(e.events.iter().map(value).collect::<Vec<_>>(), vec![f["after5002"]["first"].clone()]);
}

// Rule 3.7: a listener's emit is delivered at once, depth first; a listener removed mid-delivery is skipped and one
// added mid-delivery is reached.
#[test]
fn a_nested_emit_is_delivered_depth_first_over_a_live_listener_set() {
    let f = fixture("events");
    let bus = event_bus(STREAM.into());
    let order = log();
    let off_b: Rc<RefCell<Option<Unlisten>>> = Rc::new(RefCell::new(None));
    bus.on(ANY_EVENT, recorder(&order, "any1"));
    {
        let (bus2, order2, off_b2) = (bus.clone(), order.clone(), off_b.clone());
        let typed1 = recorder(&order, "typed1");
        bus.on(
            "workspace.renamed",
            Rc::new(move |e: &EventUnion| {
                typed1(e)?;
                if value(e)["workspaceId"] == "w1" {
                    bus2.emit(renamed(9))?;
                    if let Some(off) = off_b2.borrow().as_ref() {
                        off();
                    }
                    bus2.on("workspace.renamed", recorder(&order2, "added"));
                }
                Ok(())
            }),
        );
    }
    *off_b.borrow_mut() = Some(bus.on("workspace.renamed", recorder(&order, "typedB")));
    bus.emit(renamed(1)).unwrap();
    let want: Vec<String> = f["nested"].as_array().unwrap().iter().map(|s| s.as_str().unwrap().to_string()).collect();
    assert_eq!(*order.borrow(), want);
}

// Rule 3.6: a listener's failure stops the later listeners and reaches emit's caller; the event is already ringed.
#[test]
fn a_failing_listener_stops_delivery_and_the_event_stays_ringed() {
    let f = &fixture("events")["throwing"];
    let bus = event_bus(STREAM.into());
    let order = log();
    let o = order.clone();
    bus.on(
        "workspace.renamed",
        Rc::new(move |_e: &EventUnion| {
            o.borrow_mut().push("first".into());
            Err(broke("listener broke"))
        }),
    );
    bus.on("workspace.renamed", recorder(&order, "second"));
    bus.on(ANY_EVENT, recorder(&order, "any"));
    let err = bus.emit(renamed(1)).expect_err("the listener's failure reaches emit");
    assert_eq!(err.message, f["thrown"]["err"].as_str().unwrap());
    assert_eq!(*order.borrow(), vec!["first".to_string()]);
    assert_eq!(since_value(&bus.since(Some(0), None)), f["since"]);
}

// events.ts:72: unlisten deletes that one listener; twice is harmless and touches no other.
#[test]
fn unlisten_twice_deletes_only_its_own_listener() {
    let bus = event_bus(STREAM.into());
    let order = log();
    let off = bus.on("workspace.renamed", recorder(&order, "x"));
    off();
    off();
    bus.on("workspace.renamed", recorder(&order, "y"));
    bus.emit(renamed(1)).unwrap();
    assert_eq!(*order.borrow(), vec!["y:w1:1".to_string()]);
}

// Rule 3.20: an emptied listener set is never removed from the map.
#[test]
fn an_emptied_listener_set_stays_in_the_map() {
    let bus = event_bus(STREAM.into());
    let order = log();
    let off = bus.on("workspace.renamed", recorder(&order, "x"));
    off();
    assert!(bus.state.borrow().listeners.contains_key("workspace.renamed"));
    assert_eq!(bus.state.borrow().head, 0);
}

// Rule 3.20: emit takes a seq and fills the ring with no listener attached.
#[test]
fn emit_with_no_listener_still_takes_a_seq() {
    let bus = event_bus(STREAM.into());
    assert_eq!(bus.emit(renamed(1)).unwrap(), 1);
    assert_eq!(bus.emit(renamed(2)).unwrap(), 2);
    let s = bus.since(Some(1), None);
    assert_eq!(
        s.events.iter().map(value).collect::<Vec<_>>(),
        vec![json!({ "type": "workspace.renamed", "workspaceId": "w2", "name": "n2", "seq": 2 })]
    );
}

// events.ts:92: pass never moves the head, so a later emit takes the next number.
#[test]
fn pass_never_moves_the_head() {
    let bus = event_bus(STREAM.into());
    bus.pass(renamed(1)).unwrap();
    bus.emit(renamed(2)).unwrap();
    bus.pass(renamed(3)).unwrap();
    assert_eq!(bus.emit(renamed(4)).unwrap(), 2);
    assert_eq!(bus.since(None, None).head, 2);
}

// Rule 3.28: a new bus starts at head 0 with the stream it was given, whatever another bus has issued.
#[test]
fn a_new_bus_starts_over_under_its_own_stream() {
    let old = event_bus("old".into());
    old.emit(renamed(1)).unwrap();
    let new = event_bus("new".into());
    assert_eq!(since_value(&new.since(Some(1), Some("old"))), json!({ "stream": "new", "head": 0, "events": [], "gap": true }));
    assert_eq!(new.emit(renamed(2)).unwrap(), 1);
}

// The helpers the bus files and stamps by.
#[test]
fn event_type_seq_and_with_seq_read_and_set_the_keys() {
    let e = renamed(1);
    assert_eq!(event_type(&e), "workspace.renamed");
    assert_eq!(event_seq(&e), None);
    let s = with_seq(e, 7);
    assert_eq!(event_seq(&s), Some(7));
    assert_eq!(json_of(&s), r#"{"type":"workspace.renamed","workspaceId":"w1","name":"n1","seq":7}"#);
}

// events.ts:21-57.
#[test]
fn constants_match_the_ts() {
    let c = &fixture("events")["constants"];
    let pairs: [(&str, u64); 13] = [
        ("EVENT_RING_CAP", EVENT_RING_CAP as u64),
        ("SLATE_PR_POLL_MS", SLATE_PR_POLL_MS),
        ("CATALOG_TTL_MS", CATALOG_TTL_MS),
        ("CATALOG_PROBE_TIMEOUT_MS", CATALOG_PROBE_TIMEOUT_MS),
        ("FIRST_RUN_READ_MS", FIRST_RUN_READ_MS),
        ("AGENT_VERSION_READ_MS", AGENT_VERSION_READ_MS),
        ("SESSION_TITLE_TTL_MS", SESSION_TITLE_TTL_MS),
        ("CHECKOUT_TTL_MS", CHECKOUT_TTL_MS),
        ("PR_PAGE_HOLD_MS", PR_PAGE_HOLD_MS),
        ("RATE_LIMIT_HOLD_MS", RATE_LIMIT_HOLD_MS),
        ("SESSION_TITLE_TIMEOUT_MS", SESSION_TITLE_TIMEOUT_MS),
        ("SESSION_TITLE_REFRESH_MAX", SESSION_TITLE_REFRESH_MAX as u64),
        ("TITLE_MAKE_TIMEOUT_MS", TITLE_MAKE_TIMEOUT_MS),
    ];
    for (name, got) in pairs {
        assert_eq!(c[name].as_u64(), Some(got), "{name}");
    }
}

// events.ts:71: the listeners are a Set, so one listener added twice under a type hears each event once, and one
// unlisten takes it.
#[test]
fn the_same_listener_added_twice_hears_once() {
    let bus = event_bus(STREAM.into());
    let order = log();
    let l = recorder(&order, "x");
    let off1 = bus.on("workspace.renamed", l.clone());
    let _off2 = bus.on("workspace.renamed", l.clone());
    bus.emit(renamed(1)).unwrap();
    assert_eq!(*order.borrow(), vec!["x:w1:1".to_string()]);
    off1();
    bus.emit(renamed(2)).unwrap();
    assert_eq!(*order.borrow(), vec!["x:w1:1".to_string()]);
}

// events.ts:92: before any emit a passed event goes as given, a seq it already carries included.
#[test]
fn pass_before_any_emit_leaves_the_events_own_seq() {
    let bus = event_bus(STREAM.into());
    let order = log();
    bus.on(ANY_EVENT, recorder(&order, "any"));
    bus.pass(with_seq(renamed(1), 5)).unwrap();
    bus.emit(renamed(2)).unwrap();
    bus.pass(with_seq(renamed(3), 5)).unwrap();
    assert_eq!(*order.borrow(), vec!["any:w1:5".to_string(), "any:w2:1".into(), "any:w3:1".into()]);
}
