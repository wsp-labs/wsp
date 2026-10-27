// SPDX-License-Identifier: AGPL-3.0-only
//! `rt.events.on("*", e => events.push(e))` of the TS tests: every event kept as the JSON the wire carries, so a test
//! reads `stage`, `message` or `status.phase` as the TS does.
use std::cell::RefCell;
use std::rc::Rc;

use serde_json::Value;
use wsp_core::types::events::{EventBus, ANY_EVENT};

pub use serde_json::json;

#[derive(Clone, Default)]
pub struct Events {
    seen: Rc<RefCell<Vec<Value>>>,
}

impl Events {
    /// Listens to every event `bus` carries from now on.
    pub fn on(bus: &dyn EventBus) -> Events {
        let events = Events::default();
        let seen = events.seen.clone();
        let _unlisten = bus.on(
            ANY_EVENT,
            Rc::new(move |e| {
                seen.borrow_mut().push(serde_json::to_value(e).expect("an event"));
                Ok(())
            }),
        );
        events
    }

    pub fn all(&self) -> Vec<Value> {
        self.seen.borrow().clone()
    }

    pub fn len(&self) -> usize {
        self.seen.borrow().len()
    }

    pub fn is_empty(&self) -> bool {
        self.seen.borrow().is_empty()
    }

    /// The events of one type, oldest first.
    pub fn of(&self, kind: &str) -> Vec<Value> {
        self.seen.borrow().iter().filter(|e| e["type"] == kind).cloned().collect()
    }

    /// `events.some(e => e.type === kind)`.
    pub fn has(&self, kind: &str) -> bool {
        self.seen.borrow().iter().any(|e| e["type"] == kind)
    }

    /// The `workspace.creating` events: the steps a create reports.
    pub fn creating(&self) -> Vec<Value> {
        self.of("workspace.creating")
    }

    /// The `status` of every `workspace.status` event, oldest first.
    pub fn statuses(&self) -> Vec<Value> {
        self.of("workspace.status").into_iter().map(|e| e["status"].clone()).collect()
    }

    /// The index of the first event that `pick` holds for.
    pub fn position(&self, pick: impl Fn(&Value) -> bool) -> Option<usize> {
        self.seen.borrow().iter().position(pick)
    }
}
