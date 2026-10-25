// SPDX-License-Identifier: AGPL-3.0-only
//! The messages steered into a running turn, as the CLI reports what it does with each. TS:
//! packages/adapter-claude/src/steers.ts at a3b870a6e. Fields are the implementer's; the methods are the contract.

use indexmap::IndexMap;

use crate::claude::fields::Obj;
use crate::port::AdapterEvent;

/// steers.ts:10.
#[derive(Clone, Debug, Default)]
pub struct Steers {
    /// Each steer by uuid, in the order added, with whether the CLI started it.
    steers: IndexMap<String, bool>,
    dropped: Vec<String>,
    reports: bool,
    cancels_queued: bool,
    fixed: Option<Vec<String>>,
    told: bool,
}

impl Steers {
    /// steers.ts:38 (`steersOf`): the ids steered_ids reads off `taken`, then `kept`, each once, in that order.
    pub fn new(taken: &[String], kept: &[String]) -> Self {
        todo!()
    }
    pub fn reports(&self) -> bool {
        todo!()
    }
    pub fn cancels_queued(&self) -> bool {
        todo!()
    }
    /// Re-adding a uuid resets it to not started and keeps its place.
    pub fn add(&mut self, uuid: &str) {
        todo!()
    }
    pub fn forget(&mut self, uuid: &str) {
        todo!()
    }
    /// steers.ts:63. Reads an init's capabilities (answering false for it) and takes a command_lifecycle line (true).
    pub fn read(&mut self, event: &Obj) -> bool {
        todo!()
    }
    pub fn open(&self) -> bool {
        todo!()
    }
    pub fn working(&self) -> bool {
        todo!()
    }
    /// steers.ts:85. Fixes the unread ids the first time it runs; true where a not-started steer is still queued.
    pub fn settle(&mut self) -> bool {
        todo!()
    }
    /// steers.ts:89. One turn.unread at most per Steers, never with empty ids.
    pub fn tell_unread(&mut self, session_id: &str, on_event: &dyn Fn(AdapterEvent)) {
        todo!()
    }
}

/// steers.ts:95. The uuids of the user lines after the last user line with no uuid; lines that do not parse or are
/// not `type: "user"` are skipped; each uuid once, first place kept.
pub fn steered_ids(taken: &[String]) -> Vec<String> {
    todo!()
}
