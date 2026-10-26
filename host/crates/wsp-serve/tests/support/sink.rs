// SPDX-License-Identifier: AGPL-3.0-only
//! A socket transport that records what the socket was told.
use std::cell::{Cell, RefCell};
use std::rc::Rc;

use serde_json::Value;
use wsp_machines::places::helpers::LinkSocket;
use wsp_serve::serve::socket::FrameSink;

#[derive(Default)]
pub struct RecordingSink {
    pub texts: RefCell<Vec<String>>,
    pub binaries: RefCell<Vec<Vec<u8>>>,
    pub closes: RefCell<Vec<(u16, String)>>,
    pub terminated: Cell<bool>,
    pub paused: Cell<bool>,
    pub open: Cell<bool>,
    pub linked: Cell<bool>,
    /// The sealed frames already opened, so a second reading opens only what came after.
    pub opened: RefCell<Vec<Value>>,
}

impl RecordingSink {
    pub fn new() -> Rc<RecordingSink> {
        let sink = RecordingSink::default();
        sink.open.set(true);
        Rc::new(sink)
    }
    /// Every text frame sent, parsed.
    pub fn frames(&self) -> Vec<Value> {
        self.texts.borrow().iter().map(|t| serde_json::from_str(t).expect("a sent frame is JSON")).collect()
    }
    /// The frames that are no reply: pushes, which carry no `id`.
    pub fn pushes(&self) -> Vec<Value> {
        self.frames().into_iter().filter(|f| f.get("id").is_none()).collect()
    }
    /// The last reply carrying `id`.
    pub fn reply(&self, id: u64) -> Option<Value> {
        self.frames().into_iter().rev().find(|f| f.get("id") == Some(&Value::from(id)))
    }
    /// Every reply carrying `id`, in order.
    pub fn replies(&self, id: u64) -> Vec<Value> {
        self.frames().into_iter().filter(|f| f.get("id") == Some(&Value::from(id))).collect()
    }
    /// Every sealed reply so far, opened with the client's end of the seal; each frame opens once, as its counter moves.
    pub fn opened_with(&self, client: &mut wsp_store::keys::Seal) -> Vec<Value> {
        let binaries = self.binaries.borrow();
        let mut opened = self.opened.borrow_mut();
        for bytes in binaries.iter().skip(opened.len()) {
            let text = client.unseal(bytes).expect("a sealed reply opens");
            opened.push(serde_json::from_str(&text).expect("a sealed reply is JSON"));
        }
        opened.clone()
    }
    /// The text of the last frame sent, for key order.
    pub fn last_text(&self) -> String {
        self.texts.borrow().last().cloned().unwrap_or_default()
    }
    /// The first close the socket was told, if any.
    pub fn closed(&self) -> Option<(u16, String)> {
        self.closes.borrow().first().cloned()
    }
}

impl FrameSink for RecordingSink {
    fn is_open(&self) -> bool {
        self.open.get() && self.closes.borrow().is_empty() && !self.terminated.get()
    }
    fn send_text(&self, text: String) {
        self.texts.borrow_mut().push(text);
    }
    fn send_binary(&self, bytes: Vec<u8>) {
        self.binaries.borrow_mut().push(bytes);
    }
    fn close(&self, code: u16, reason: &str) {
        self.closes.borrow_mut().push((code, reason.to_string()));
    }
    fn terminate(&self) {
        self.terminated.set(true);
    }
    fn pause(&self) {
        self.paused.set(true);
    }
    fn link(self: Rc<Self>) -> Rc<dyn LinkSocket> {
        self.linked.set(true);
        Rc::new(NoLink)
    }
}

/// The link a proved place's socket becomes; the place door's fake only checks it was handed one.
pub struct NoLink;

impl LinkSocket for NoLink {
    fn is_open(&self) -> bool {
        true
    }
    fn close(&self, _code: u16, _reason: &str) {}
    fn on_close(&self, _f: Box<dyn FnOnce(u16, String)>) -> u64 {
        0
    }
    fn off_close(&self, _id: u64) {}
    fn connect(
        self: Rc<Self>,
        _seal: Option<wsp_store::keys::Seal>,
        _on_event: Rc<dyn Fn(&wsp_wire::wire::daemon_events::DaemonEvent)>,
    ) -> Rc<dyn wsp_core::reach::DaemonReach> {
        panic!("the fake link is never dialled")
    }
}
