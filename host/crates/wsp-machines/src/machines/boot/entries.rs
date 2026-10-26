// SPDX-License-Identifier: AGPL-3.0-only
//! boot.ts:623-672,1014-1065 at 65f6b89c7: the doors every verb comes through. Spec: spec/boot_entries.md.

use std::rc::Rc;

use wsp_core::types::internal::{label_of, ThreadRecord, TreeTalk};
use wsp_wire::views::place_kind::PlaceKind;
use wsp_wire::views::session::SessionView;
use wsp_wire::views::workspace::Caller;
use wsp_words::exit::{not_found_refusal, HostError, Kind};
use wsp_words::projects::{workspace_lands, WorkspaceLands};
use wsp_words::tree::not_the_leads_child_refusal;
use wsp_words::words::computer::{no_workspace_refusal, AcrossAct};
use wsp_words::words::image::NO_IMAGE_YET;

use super::{BootArea, ComputerRow};
use crate::machines::machines::Entry;
use crate::machines::machines::ThreadOn;

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

/// The start's one result, as an error a verb answers with (boot.ts:329).
async fn started(area: &Rc<BootArea>) -> Result<(), HostError> {
    area.ready().await.map_err(|e| e.0)?;
    tokio::task::yield_now().await;
    Ok(())
}

impl BootArea {
    /// `entryOf` (boot.ts:628), spec rule 1.
    pub async fn entry_of(
        self: &Rc<Self>,
        id: &str,
        origin: Option<&Caller>,
        ask: EntryAsk,
    ) -> Result<Entry, HostError> {
        started(self).await?;
        let entry = self.rt.live().borrow().get(id).cloned();
        let Some(entry) = entry.filter(|e| !(e.borrow().creating && !ask.being_made)) else {
            let line = no_workspace_refusal(None);
            let line = if wsp_wire::views::workspace::scope_of(origin).is_some() {
                line
            } else {
                format!("{line}: {id}")
            };
            return Err(not_found_refusal(&line));
        };
        let thread = match ask.thread.as_deref() {
            Some(t)
                if self
                    .rt
                    .thread_records()
                    .borrow()
                    .get(t)
                    .is_some_and(|r| r.workspace_id == id)
                    && self.rt.drives_thread(Some(t), origin) =>
            {
                Some(t)
            }
            _ => None,
        };
        let record = entry.borrow().record.borrow().clone();
        self.rt
            .refuse_relayed(&record, origin, ask.act.as_ref().map(|a| a.words()), thread)?;
        if ask.now {
            return Ok(entry);
        }
        let pending = self.rt.boot_work().borrow().get(id).cloned();
        match pending {
            Some(pending) => {
                pending.await;
            }
            None => tokio::task::yield_now().await,
        }
        let live = self.rt.live().borrow().get(id).cloned();
        Ok(live.unwrap_or(entry))
    }

    /// `childOf` (boot.ts:643), spec rule 2.
    pub async fn child_of(
        self: &Rc<Self>,
        lead: &Entry,
        reference: &str,
        origin: Option<&Caller>,
        lead_thread: Option<&str>,
        child_thread: Option<&str>,
    ) -> Result<Entry, HostError> {
        let named = self.rt.resolve_workspace(reference, origin).await?;
        let kid = self
            .entry_of(
                &named,
                origin,
                EntryAsk {
                    thread: child_thread.map(str::to_string),
                    ..EntryAsk::default()
                },
            )
            .await?;
        let (kid_parent, kid_label) = {
            let k = kid.borrow();
            let r = k.record.borrow();
            (
                r.lead
                    .as_ref()
                    .map(|l| l.workspace_id.clone())
                    .or_else(|| r.parent_workspace_id.clone()),
                label_of(&r.name, child_thread),
            )
        };
        let (lead_id, lead_label) = {
            let l = lead.borrow();
            let r = l.record.borrow();
            (r.id.clone(), label_of(&r.name, lead_thread))
        };
        if kid_parent.as_deref() != Some(lead_id.as_str()) {
            return Err(HostError::with_kind(
                not_the_leads_child_refusal(&kid_label, &lead_label),
                Kind::Invalid,
            ));
        }
        Ok(kid)
    }

    /// `reachesRow` (boot.ts:656), spec rule 3.
    pub fn reaches_row(
        &self,
        row: &RowAt,
        caller: Option<&Caller>,
        talk: Option<TreeTalk>,
    ) -> bool {
        if !self.rt.drives_thread(row.thread_id.as_deref(), caller) {
            return false;
        }
        let record = self
            .rt
            .live()
            .borrow()
            .get(&row.workspace_id)
            .map(|e| e.borrow().record.borrow().clone());
        match record {
            None => true,
            Some(record) => {
                (talk.is_some() && self.rt.talks_to_its_tree(caller))
                    || self.rt.acts_on(&record, caller)
            }
        }
    }

    /// `entryOfRow` (boot.ts:665), spec rule 4.
    pub async fn entry_of_row(
        self: &Rc<Self>,
        row: &RowAt,
        origin: Option<&Caller>,
        talk: Option<TreeTalk>,
        being_made: bool,
    ) -> Result<Option<Entry>, HostError> {
        if wsp_wire::views::workspace::scope_of(origin).is_none() {
            return self
                .entry_of(
                    &row.workspace_id,
                    origin,
                    EntryAsk {
                        being_made,
                        ..EntryAsk::default()
                    },
                )
                .await
                .map(Some);
        }
        started(self).await?;
        let entry = self.rt.live().borrow().get(&row.workspace_id).cloned();
        let Some(entry) = entry else { return Ok(None) };
        let creating = entry.borrow().creating;
        if (creating && !being_made) || !self.reaches_row(row, origin, talk) {
            return Ok(None);
        }
        let pending = self.rt.boot_work().borrow().get(&row.workspace_id).cloned();
        match pending {
            Some(pending) => {
                pending.await;
            }
            None => tokio::task::yield_now().await,
        }
        let live = self.rt.live().borrow().get(&row.workspace_id).cloned();
        Ok(Some(live.unwrap_or(entry)))
    }

    /// `treeRecords` (boot.ts:1017), spec rule 5: `(thread id, workspace id, record)` in map order.
    pub fn tree_records(
        &self,
        caller: Option<&Caller>,
        talk: Option<TreeTalk>,
        workspace_id: Option<&str>,
    ) -> Vec<(String, String, ThreadRecord)> {
        let root = wsp_wire::views::workspace::scope_of(caller).map(|s| s.root_thread_id);
        let records: Vec<(String, ThreadOn)> = self
            .rt
            .thread_records()
            .borrow()
            .iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();
        records
            .into_iter()
            .filter(|(thread, r)| {
                let root_ok = root.as_ref().is_none_or(|root| {
                    r.record.root_thread_id.as_deref().unwrap_or(thread) == root.as_str()
                });
                let ws_ok = workspace_id.is_none_or(|ws| ws == r.workspace_id);
                let live = self.rt.live().borrow().contains_key(&r.workspace_id);
                root_ok
                    && ws_ok
                    && live
                    && self.reaches_row(
                        &RowAt {
                            thread_id: Some(thread.clone()),
                            workspace_id: r.workspace_id.clone(),
                        },
                        caller,
                        talk,
                    )
            })
            .map(|(thread, r)| (thread, r.workspace_id.clone(), r.record))
            .collect()
    }

    /// `treeStandsOn` (boot.ts:1024), spec rule 6.
    pub fn tree_stands_on(
        &self,
        workspace_id: &str,
        caller: Option<&Caller>,
        talk: Option<TreeTalk>,
    ) -> bool {
        if wsp_wire::views::workspace::scope_of(caller).is_none() {
            return false;
        }
        let views: Vec<SessionView> = self
            .rt
            .sessions()
            .borrow()
            .values()
            .map(|s| s.borrow().view.clone())
            .collect();
        let by_row = views.iter().any(|v| {
            v.workspace_id == workspace_id
                && self.reaches_row(
                    &RowAt {
                        thread_id: v.thread_id.clone(),
                        workspace_id: v.workspace_id.clone(),
                    },
                    caller,
                    talk,
                )
        });
        by_row
            || !self
                .tree_records(caller, talk, Some(workspace_id))
                .is_empty()
    }

    /// `talksToTreeOn` (boot.ts:1029), spec rule 7.
    pub fn talks_to_tree_on(&self, workspace_id: &str, caller: Option<&Caller>) -> bool {
        self.rt.talks_to_its_tree(caller)
            && self.tree_stands_on(workspace_id, caller, Some(TreeTalk::Send))
    }

    /// `computerRows` (boot.ts:1038), spec rule 8.
    pub async fn computer_rows(&self) -> Vec<ComputerRow> {
        let mut rows = match self.rt.place_rows().await {
            Some(rows) => rows,
            None => vec![ComputerRow {
                id: wsp_wire::types::place_word::HERE_PLACE_ID.to_string(),
                name: self.rt.hostname(),
                kind: PlaceKind::Computer,
            }],
        };
        let wired = self.rt.places().wired();
        if !rows.iter().any(|r| r.id == wired) {
            rows.push(ComputerRow {
                id: wired.clone(),
                name: wired,
                kind: PlaceKind::Provider,
            });
        }
        rows
    }

    /// `nameOfComputer` (boot.ts:1047), spec rule 9.
    pub fn name_of_computer(computer: &str, rows: &[ComputerRow]) -> String {
        rows.iter()
            .find(|r| r.id == computer)
            .map_or_else(|| computer.to_string(), |r| r.name.clone())
    }

    /// `imageHeadOrNone` (boot.ts:1051), spec rule 10.
    pub async fn image_head_or_none(&self) -> Result<Option<String>, HostError> {
        self.rt.default_image_head().await
    }

    /// `imageHead` (boot.ts:1054): `NO_IMAGE_YET` for none.
    pub async fn image_head(&self) -> Result<String, HostError> {
        self.image_head_or_none()
            .await?
            .ok_or_else(|| HostError::new(NO_IMAGE_YET))
    }

    /// `landingPlace` (boot.ts:1062), spec rule 11: the place id, or `None` for this computer and the wired provider.
    pub async fn landing_place(&self, computer: &str) -> Result<Option<String>, HostError> {
        let wired = self.rt.places().wired();
        match workspace_lands(computer, Some(&wired)) {
            WorkspaceLands::Place { place } => self.rt.place_for(&place).await,
            WorkspaceLands::Here | WorkspaceLands::Wired => Ok(None),
        }
    }
}
