// SPDX-License-Identifier: AGPL-3.0-only
//! The scripted answers and the call log every fake shares. A fake method records its call with its arguments as
//! JSON, then answers what the test scripted for that name: a queued answer first, else a standing one, else the
//! refusal `unscripted call: <name>`.
use std::any::Any;
use std::cell::RefCell;
use std::collections::{HashMap, VecDeque};
use std::rc::Rc;

use serde_json::Value;
use wsp_words::exit::HostError;

/// One call a fake was asked, by the TS member's dotted name (`workspaces.get`, `places.list`, `devices.match`).
#[derive(Clone, Debug, PartialEq)]
pub struct Call {
    pub name: String,
    pub args: Value,
}

type Standing = Rc<dyn Fn(&Call) -> Box<dyn Any>>;

#[derive(Default)]
pub struct Script {
    calls: RefCell<Vec<Call>>,
    queued: RefCell<HashMap<String, VecDeque<Box<dyn Any>>>>,
    standing: RefCell<HashMap<String, Standing>>,
}

impl Script {
    pub fn new() -> Rc<Script> {
        Rc::new(Script::default())
    }

    /// Queues one answer for the next call of `name`. `T` is the method's whole return type, `Result` included.
    pub fn once<T: 'static>(&self, name: &str, answer: T) {
        self.queued.borrow_mut().entry(name.to_string()).or_default().push_back(Box::new(answer));
    }

    /// Answers every call of `name` that finds nothing queued.
    pub fn on<T: 'static>(&self, name: &str, answer: impl Fn(&Call) -> T + 'static) {
        self.standing.borrow_mut().insert(name.to_string(), Rc::new(move |call| Box::new(answer(call))));
    }

    /// Records a call. Fakes call this first, then `answer`.
    pub fn record(&self, name: &str, args: Value) -> Call {
        let call = Call { name: name.to_string(), args };
        self.calls.borrow_mut().push(call.clone());
        call
    }

    /// The scripted answer for `call`, or `None` when nothing is scripted. Panics when one is scripted under another
    /// type, which is a test written wrong.
    pub fn answer<T: 'static>(&self, call: &Call) -> Option<T> {
        let queued = self.queued.borrow_mut().get_mut(&call.name).and_then(|q| q.pop_front());
        let boxed = match queued {
            Some(b) => b,
            None => {
                let standing = self.standing.borrow().get(&call.name).cloned()?;
                standing(call)
            }
        };
        Some(*boxed.downcast::<T>().unwrap_or_else(|_| panic!("the answer scripted for {} has another type", call.name)))
    }

    /// `record` then `answer`, falling back to the unscripted refusal for a `Result`.
    pub fn call<T: 'static>(&self, name: &str, args: Value) -> Result<T, HostError> {
        let call = self.record(name, args);
        match self.answer::<Result<T, HostError>>(&call) {
            Some(answer) => answer,
            None => Err(HostError::new(format!("unscripted call: {name}"))),
        }
    }

    /// `record` then `answer`, falling back to `fallback` for a method that cannot fail.
    pub fn call_or<T: 'static>(&self, name: &str, args: Value, fallback: impl FnOnce() -> T) -> T {
        let call = self.record(name, args);
        self.answer::<T>(&call).unwrap_or_else(fallback)
    }

    pub fn calls(&self) -> Vec<Call> {
        self.calls.borrow().clone()
    }

    pub fn calls_to(&self, name: &str) -> Vec<Call> {
        self.calls.borrow().iter().filter(|c| c.name == name).cloned().collect()
    }

    pub fn names(&self) -> Vec<String> {
        self.calls.borrow().iter().map(|c| c.name.clone()).collect()
    }
}

/// An origin as the call log writes it: `null`, `"here"`, or `{"origin": .., "by": {..}}`.
pub fn caller_json(origin: &Option<wsp_wire::views::workspace::Caller>) -> Value {
    use wsp_wire::views::workspace::Caller;
    match origin {
        None => Value::Null,
        Some(Caller::Road(road)) => serde_json::to_value(road).unwrap(),
        Some(Caller::Scoped { origin, by }) => serde_json::json!({ "origin": origin, "by": by }),
    }
}
