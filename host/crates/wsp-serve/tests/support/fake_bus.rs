// SPDX-License-Identifier: AGPL-3.0-only
//! A bus the test emits on by hand. `since` answers what the test scripted under `events.since` (default: stream
//! "stream-a", head 0, nothing, no gap); `emit` hands the event to every listener of its type and of `*`, in the
//! order they were added.
use std::cell::RefCell;
use std::rc::Rc;

use serde_json::{json, Value};
use wsp_core::types::events::{EventBus, EventListener, Since, Unlisten, ANY_EVENT};
use wsp_wire::wire::events_union::EventUnion;

use super::script::Script;

pub struct FakeBus {
    pub script: Rc<Script>,
    listeners: Rc<RefCell<Vec<(u64, String, EventListener)>>>,
    next: RefCell<u64>,
}

impl FakeBus {
    pub fn new(script: Rc<Script>) -> Rc<FakeBus> {
        Rc::new(FakeBus { script, listeners: Rc::new(RefCell::new(Vec::new())), next: RefCell::new(0) })
    }
    /// An event written as the wire writes it, e.g. `{"type":"session.delta",...}`.
    pub fn event(v: Value) -> EventUnion {
        serde_json::from_value(v).expect("a test event parses as the wire's union")
    }
    pub fn emit(&self, e: &EventUnion) {
        let ty = serde_json::to_value(e).unwrap()["type"].as_str().unwrap_or_default().to_string();
        let now: Vec<EventListener> = self.listeners.borrow().iter().filter(|(_, t, _)| t == ANY_EVENT || *t == ty).map(|(_, _, l)| l.clone()).collect();
        for l in now {
            let _ = l(e);
        }
    }
    /// How many listeners stand now.
    pub fn listening(&self) -> usize {
        self.listeners.borrow().len()
    }
}

impl EventBus for FakeBus {
    fn on(&self, event_type: &str, listener: EventListener) -> Unlisten {
        let id = {
            let mut n = self.next.borrow_mut();
            *n += 1;
            *n
        };
        self.script.record("events.on", json!({ "type": event_type }));
        self.listeners.borrow_mut().push((id, event_type.to_string(), listener));
        let listeners = self.listeners.clone();
        Box::new(move || listeners.borrow_mut().retain(|(i, _, _)| *i != id))
    }
    fn since(&self, after: Option<u64>, stream: Option<&str>) -> Since {
        self.script.call_or("events.since", json!({ "after": after, "stream": stream }), || Since { stream: "stream-a".into(), head: 0, events: vec![], gap: false })
    }
}
