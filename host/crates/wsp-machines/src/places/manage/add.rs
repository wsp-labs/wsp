// SPDX-License-Identifier: AGPL-3.0-only
//! manage.ts:197-423 at 65f6b89c7: add, dial, road, exec, adds, reportOf, homeOf. Spec: `spec/places_manage_add.md`
//! (rules A1-A15, D1-D4, RD1-RD2, E1-E3, AD1-AD2).

use std::rc::Rc;

use wsp_engine::machine::ExecResult;
use wsp_wire::views::place::PlaceAddJob;
use wsp_wire::views::place_more::PlaceDial;
use wsp_wire::wire::place_link::PlaceReport;

use super::ManageDoor;
use crate::places::types::{AddAsk, PlaceAdded, PlaceExecOpts, PlaceResult};

impl ManageDoor {
    /// A1-A15 (:198-342).
    pub async fn add(self: &Rc<Self>, req: AddAsk, at: u64) -> PlaceResult<PlaceAdded> {
        todo!()
    }

    /// D1-D4 (:344-382).
    pub async fn dial(&self, place_id: &str, at: u64) -> PlaceResult<PlaceDial> {
        todo!()
    }

    /// RD1-RD2 (:384-398).
    pub async fn road(&self, place_id: &str) -> PlaceResult<u16> {
        todo!()
    }

    /// E1-E3 (:400-418).
    pub async fn exec(&self, place_id: &str, cmd: &str, opts: PlaceExecOpts) -> PlaceResult<ExecResult> {
        todo!()
    }

    /// AD1 (:420).
    pub fn adds(&self) -> Vec<PlaceAddJob> {
        todo!()
    }

    /// AD2 (:422).
    pub async fn report_of(&self, place_id: &str) -> PlaceResult<Option<PlaceReport>> {
        todo!()
    }

    /// AD2 (:423).
    pub async fn home_of(&self, place_id: &str) -> PlaceResult<Option<String>> {
        todo!()
    }
}
