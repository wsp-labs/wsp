// SPDX-License-Identifier: AGPL-3.0-only
//! manage.ts:90-195,494-720 at 65f6b89c7: holds, remove, update and their helpers. Spec:
//! `spec/places_manage_remove.md` (rules H1-H6, HL1, RM1-RM15, U1-U6).

use wsp_wire::views::place_more::{PlaceHolds, PlaceHoldsImagesItem, PlaceRemoved, PlaceUpdateReply};

use super::ManageDoor;
use crate::places::types::{PlaceLogin, PlaceRecord, PlaceResult, RemoveAsk};

/// How long a leave over the link gives the threads' cgroups there to end (manage.ts:91).
pub const THREADS_END_MS: u64 = 60_000;

/// Where an older wsp kept a project's checkout on a computer you joined (manage.ts:94): `/wsp/projects`.
pub fn runtime_checkouts() -> String {
    todo!()
}

/// H1 (:97).
pub fn runtime_checkouts_script() -> String {
    todo!()
}

/// H2 (:101-104).
pub fn runtime_project_of(checkout: Option<&str>) -> Option<String> {
    todo!()
}

/// What `reaches_itself` answers (H3).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ReachesItself {
    Stands,
    Refused { refused: String },
    Elsewhere { other: Option<String> },
}

impl ManageDoor {
    /// H3 (:151-157).
    pub async fn reaches_itself(&self, record: &PlaceRecord, login: &PlaceLogin, sudo_password: Option<&str>) -> PlaceResult<ReachesItself> {
        todo!()
    }

    /// H4 (:162-168).
    pub async fn holds_of(&self, place_id: &str, record: &PlaceRecord) -> PlaceResult<PlaceHolds> {
        todo!()
    }

    /// H6 (:172-182).
    pub async fn images_sized(&self, place_id: &str) -> PlaceResult<Vec<PlaceHoldsImagesItem>> {
        todo!()
    }

    /// H5 (:184-195).
    pub async fn unrecorded_unsaved(&self, place_id: &str) -> PlaceResult<Vec<String>> {
        todo!()
    }

    /// HL1 (:560-568).
    pub async fn holds(&self, place_id: &str) -> PlaceResult<PlaceHolds> {
        todo!()
    }

    /// RM1-RM15 (:570-720).
    pub async fn remove(&self, place_id: &str, ask: RemoveAsk) -> PlaceResult<PlaceRemoved> {
        todo!()
    }

    /// U1-U6 (:494-558).
    pub async fn update(&self, place_id: &str, ask: RemoveAsk) -> PlaceResult<PlaceUpdateReply> {
        todo!()
    }
}
