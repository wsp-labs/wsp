// SPDX-License-Identifier: AGPL-3.0-only
//! reach.ts:1-197, 374-379, 476-482: the status pushed outside the poll, the refused nap and stop, and the disk of a
//! machine whose stop snapshots it. Spec: spec/reach_disk.md.
use wsp_engine::machine::MachineBackend;
use wsp_wire::views::workspace::ReachState;
use wsp_words::words::thread::DiskMove;

use super::ReachArea;
use crate::machines::area_error::AreaResult;
use crate::machines::machines::Entry;
use wsp_core::types::internal::SessionEntry;

/// reach.ts:15. How often a running machine whose stop snapshots its disk has it read.
pub const DISK_READ_MS: f64 = 120_000.0;
/// reach.ts:19. How long a host closing waits on a disk read still out.
pub const DISK_CLOSE_WAIT_MS: f64 = 2_000.0;

/// What one disk reading answers (reach.ts:63): the reading and whether it crossed the line.
#[derive(Clone, Debug, PartialEq)]
pub struct DiskReading {
    pub pct: f64,
    pub used_bytes: f64,
    pub free_bytes: f64,
    pub crossed: bool,
}

impl ReachArea {
    /// reach.ts:35 (rule 2).
    pub fn nap_refused_of(&self, entry: &Entry) -> Option<String> {
        todo!()
    }

    /// reach.ts:42 (rule 2).
    pub fn nap_refused_reason(&self, entry: &Entry) -> Option<String> {
        todo!()
    }

    /// reach.ts:49 (rule 3).
    pub fn stop_refused_reason(&self, entry: &Entry) -> Option<String> {
        todo!()
    }

    /// reach.ts:56 (rule 4).
    pub fn disk_reason(&self, entry: &Entry) -> Option<String> {
        todo!()
    }

    /// reach.ts:60 (rule 5).
    pub(crate) fn watches_disk(&self, entry: &Entry) -> bool {
        todo!()
    }

    /// reach.ts:63 (rule 6).
    pub(crate) async fn disk_read(&self, entry: &Entry) -> Option<DiskReading> {
        todo!()
    }

    /// reach.ts:78 (rule 7).
    pub async fn read_disk(&self, entry: &Entry) {
        todo!()
    }

    /// reach.ts:88 (rule 8).
    pub(crate) async fn read_disk_now(&self, entry: &Entry) {
        todo!()
    }

    /// reach.ts:107 (rule 12).
    pub fn tell_threads(&self, workspace_id: &str, line: &dyn Fn(&str) -> String) {
        todo!()
    }

    /// reach.ts:116 (rule 11). The move less its thread.
    pub(crate) async fn move_for(&self, entry: &Entry, read_bytes: f64) -> Option<DiskMove> {
        todo!()
    }

    /// reach.ts:128 (rule 13).
    pub(crate) fn last_turns(&self, workspace_id: &str) -> Vec<SessionEntry> {
        todo!()
    }

    /// reach.ts:143 (rule 9).
    pub fn arm_disk_watch(&self) {
        todo!()
    }

    /// reach.ts:155 (rule 10).
    pub async fn stop_disk_watch(&self) {
        todo!()
    }

    /// reach.ts:165 (rule 14).
    pub fn reach_of(&self, entry: &Entry) -> ReachState {
        todo!()
    }

    /// reach.ts:172 (rule 15).
    pub async fn emit_status(&self, entry: &Entry, reach: ReachState, reason: Option<&str>) -> AreaResult<()> {
        todo!()
    }

    /// reach.ts:194 (rule 16).
    pub async fn says_waking(&self, entry: &Entry, words: &str) -> AreaResult<()> {
        todo!()
    }

    /// reach.ts:376 (rule 17).
    pub async fn push_status(&self, entry: &Entry) -> AreaResult<()> {
        todo!()
    }

    /// reach.ts:477 (rule 17).
    pub async fn status_now(&self, entry: &Entry) -> AreaResult<()> {
        todo!()
    }
}

#[allow(dead_code)]
fn _uses(_: &dyn MachineBackend) {}
