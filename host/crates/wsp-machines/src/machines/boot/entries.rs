// SPDX-License-Identifier: AGPL-3.0-only
//! boot.ts:623-672,1014-1065 at 65f6b89c7: the doors every verb comes through. Spec: spec/boot_entries.md.

use std::rc::Rc;

use wsp_core::types::internal::{ThreadRecord, TreeTalk};
use wsp_wire::views::workspace::Caller;
use wsp_words::exit::HostError;
use wsp_words::words::computer::AcrossAct;

use super::{BootArea, ComputerRow};
use crate::machines::machines::Entry;

/// What a verb asks of `entry_of` beside the id (boot.ts:628).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct EntryAsk {
    pub now: bool,
    pub act: Option<AcrossAct>,
    pub thread: Option<String>,
    pub being_made: bool,
}

/// A row as the reach rules read it (boot.ts:656).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RowAt {
    pub thread_id: Option<String>,
    pub workspace_id: String,
}

impl BootArea {
    /// `entryOf` (boot.ts:628), spec rule 1.
    pub async fn entry_of(self: &Rc<Self>, id: &str, origin: Option<&Caller>, ask: EntryAsk) -> Result<Entry, HostError> {
        todo!()
    }

    /// `childOf` (boot.ts:643), spec rule 2.
    pub async fn child_of(self: &Rc<Self>, lead: &Entry, reference: &str, origin: Option<&Caller>, lead_thread: Option<&str>, child_thread: Option<&str>) -> Result<Entry, HostError> {
        todo!()
    }

    /// `reachesRow` (boot.ts:656), spec rule 3.
    pub fn reaches_row(&self, row: &RowAt, caller: Option<&Caller>, talk: Option<TreeTalk>) -> bool {
        todo!()
    }

    /// `entryOfRow` (boot.ts:665), spec rule 4.
    pub async fn entry_of_row(self: &Rc<Self>, row: &RowAt, origin: Option<&Caller>, talk: Option<TreeTalk>, being_made: bool) -> Result<Option<Entry>, HostError> {
        todo!()
    }

    /// `treeRecords` (boot.ts:1017), spec rule 5: `(thread id, workspace id, record)` in map order.
    pub fn tree_records(&self, caller: Option<&Caller>, talk: Option<TreeTalk>, workspace_id: Option<&str>) -> Vec<(String, String, ThreadRecord)> {
        todo!()
    }

    /// `treeStandsOn` (boot.ts:1024), spec rule 6.
    pub fn tree_stands_on(&self, workspace_id: &str, caller: Option<&Caller>, talk: Option<TreeTalk>) -> bool {
        todo!()
    }

    /// `talksToTreeOn` (boot.ts:1029), spec rule 7.
    pub fn talks_to_tree_on(&self, workspace_id: &str, caller: Option<&Caller>) -> bool {
        todo!()
    }

    /// `computerRows` (boot.ts:1038), spec rule 8.
    pub async fn computer_rows(&self) -> Vec<ComputerRow> {
        todo!()
    }

    /// `nameOfComputer` (boot.ts:1047), spec rule 9.
    pub fn name_of_computer(computer: &str, rows: &[ComputerRow]) -> String {
        todo!()
    }

    /// `imageHeadOrNone` (boot.ts:1051), spec rule 10.
    pub async fn image_head_or_none(&self) -> Result<Option<String>, HostError> {
        todo!()
    }

    /// `imageHead` (boot.ts:1054): `NO_IMAGE_YET` for none.
    pub async fn image_head(&self) -> Result<String, HostError> {
        todo!()
    }

    /// `landingPlace` (boot.ts:1062), spec rule 11: the place id, or `None` for this computer and the wired provider.
    pub async fn landing_place(&self, computer: &str) -> Result<Option<String>, HostError> {
        todo!()
    }
}
