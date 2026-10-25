// SPDX-License-Identifier: AGPL-3.0-only
//! `console.warn`, which every runtime file calls: one line to stderr. Tests capture the lines on their own thread.
use std::cell::RefCell;
use std::rc::Rc;

/// Writes `line` to stderr, and to every capture open on this thread.
pub fn warn(line: &str) {
    todo!()
}

/// The lines warned on this thread while it is held; dropping it stops the capture.
pub struct Captured {
    lines: Rc<RefCell<Vec<String>>>,
}

/// Starts capturing this thread's warn lines.
pub fn capture() -> Captured {
    todo!()
}

impl Captured {
    pub fn lines(&self) -> Vec<String> {
        todo!()
    }
}

impl Drop for Captured {
    fn drop(&mut self) {
        todo!()
    }
}
