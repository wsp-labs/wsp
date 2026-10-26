// SPDX-License-Identifier: AGPL-3.0-only
//! The messages steered into a running turn, as the CLI reports what it does with each. TS:
//! packages/adapter-claude/src/steers.ts at a3b870a6e. Fields are the implementer's; the methods are the contract.

use indexmap::IndexMap;
use serde_json::Value;

use crate::claude::fields::{str_arr, string, Obj};
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
        let mut steers = IndexMap::new();
        for uuid in steered_ids(taken).iter().chain(kept) {
            steers.insert(uuid.clone(), false);
        }
        Steers { steers, ..Steers::default() }
    }
    pub fn reports(&self) -> bool {
        self.reports
    }
    pub fn cancels_queued(&self) -> bool {
        self.cancels_queued
    }
    /// Re-adding a uuid resets it to not started and keeps its place.
    pub fn add(&mut self, uuid: &str) {
        self.steers.insert(uuid.to_string(), false);
    }
    pub fn forget(&mut self, uuid: &str) {
        self.steers.shift_remove(uuid);
    }
    /// steers.ts:63. Reads an init's capabilities (answering false for it) and takes a command_lifecycle line (true).
    pub fn read(&mut self, event: &Obj) -> bool {
        if string(event.get("type")) == Some("system") && string(event.get("subtype")) == Some("init") {
            let capabilities = str_arr(event.get("capabilities")).unwrap_or_default();
            self.reports |= capabilities.iter().any(|c| c == "msg_lifecycle_v1");
            self.cancels_queued |= capabilities.iter().any(|c| c == "interrupt_cancel_queued_v1");
        }
        if string(event.get("type")) != Some("command_lifecycle") {
            return false;
        }
        self.reports = true;
        let Some(uuid) = string(event.get("command_uuid")) else {
            return true;
        };
        let Some(&started) = self.steers.get(uuid) else {
            return true;
        };
        match string(event.get("state")) {
            Some("queued") => {}
            Some("started") => {
                self.steers.insert(uuid.to_string(), true);
            }
            _ => {
                self.steers.shift_remove(uuid);
                if !started {
                    self.dropped.push(uuid.to_string());
                }
            }
        }
        true
    }
    pub fn open(&self) -> bool {
        self.reports && !self.steers.is_empty()
    }
    pub fn working(&self) -> bool {
        self.steers.values().any(|started| *started)
    }
    fn not_started(&self) -> Vec<String> {
        self.steers.iter().filter(|(_, started)| !**started).map(|(id, _)| id.clone()).collect()
    }
    /// steers.ts:85. Fixes the unread ids the first time it runs; true where a not-started steer is still queued.
    pub fn settle(&mut self) -> bool {
        if self.fixed.is_none() {
            self.fixed = Some(if self.reports { self.dropped.iter().cloned().chain(self.not_started()).collect() } else { Vec::new() });
        }
        self.reports && !self.not_started().is_empty()
    }
    /// steers.ts:89. One turn.unread at most per Steers, never with empty ids.
    pub fn tell_unread(&mut self, session_id: &str, on_event: &dyn Fn(AdapterEvent)) {
        let ids = match &self.fixed {
            _ if self.told => Vec::new(),
            Some(fixed) => fixed.clone(),
            None if self.reports => self.dropped.iter().cloned().chain(self.not_started()).collect(),
            None => Vec::new(),
        };
        self.told = true;
        if !ids.is_empty() {
            on_event(AdapterEvent::TurnUnread { session_id: session_id.to_string(), ids });
        }
    }
}

/// steers.ts:95. The uuids of the user lines after the last user line with no uuid; lines that do not parse or are
/// not `type: "user"` are skipped; each uuid once, first place kept.
pub fn steered_ids(taken: &[String]) -> Vec<String> {
    let mut ids: Vec<String> = Vec::new();
    for line in taken {
        let Some(Value::Object(message)) = wsp_words::v8_json::parse_json_lax(line) else {
            continue;
        };
        if string(message.get("type")) != Some("user") {
            continue;
        }
        match string(message.get("uuid")) {
            None => ids.clear(),
            Some(uuid) if !ids.iter().any(|id| id == uuid) => ids.push(uuid.to_string()),
            Some(_) => {}
        }
    }
    ids
}
