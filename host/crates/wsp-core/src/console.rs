// SPDX-License-Identifier: AGPL-3.0-only
//! `console.warn`, which every runtime file calls: one line to stderr. Tests capture the lines on their own thread.
use std::cell::RefCell;
use std::io::Write;
use std::rc::Rc;

/// Writes `line` to stderr, a closed one dropped as node does, and to every capture open on this thread.
pub fn warn(line: &str) {
    let _ = writeln!(std::io::stderr(), "{line}");
    CAPTURES.with(|c| {
        for open in c.borrow().iter() {
            open.borrow_mut().push(line.to_string());
        }
    });
}

thread_local! {
    static CAPTURES: RefCell<Vec<Rc<RefCell<Vec<String>>>>> = const { RefCell::new(Vec::new()) };
}

/// The lines warned on this thread while it is held; dropping it stops the capture.
pub struct Captured {
    lines: Rc<RefCell<Vec<String>>>,
}

/// Starts capturing this thread's warn lines.
pub fn capture() -> Captured {
    let lines = Rc::new(RefCell::new(Vec::new()));
    CAPTURES.with(|c| c.borrow_mut().push(lines.clone()));
    Captured { lines }
}

impl Captured {
    pub fn lines(&self) -> Vec<String> {
        self.lines.borrow().clone()
    }
}

impl Drop for Captured {
    fn drop(&mut self) {
        CAPTURES.with(|c| c.borrow_mut().retain(|open| !Rc::ptr_eq(open, &self.lines)));
    }
}
