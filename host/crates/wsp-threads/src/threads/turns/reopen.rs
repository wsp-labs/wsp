// SPDX-License-Identifier: AGPL-3.0-only
//! turns.ts:743-866: the sweep of runs no thread holds, and the re-open of a turn the store left running.
//! Spec: spec/turns_reopen.md.
use wsp_core::types::internal::Reopened;

use super::{ReopenRow, TurnsArea};
use crate::pending::Entry;

impl TurnsArea {
    /// turns.ts:748. Rules P1-P3.
    pub async fn sweep_runs(&self, entry: &Entry) {
        todo!()
    }

    /// turns.ts:774. `again` is a second try at the same row. Rules P4-P9.
    pub async fn reattach(&self, row: ReopenRow, again: bool) -> Reopened {
        todo!()
    }
}
