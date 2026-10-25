// SPDX-License-Identifier: AGPL-3.0-only
//! Port of `packages/runtime/src/places/views.ts` at 65f6b89c7: pending adds, what a remove takes off a computer, the
//! link's tunnels, and the rows a client reads. Spec: `spec/places_views.md` (rules V1-V22).
//!
//! What views reads off places-a's setup area and off `ctx.door` comes through `ViewsSetup` and `LinkReads`, so a test
//! hands fakes for both.

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;

use async_trait::async_trait;
use indexmap::{IndexMap, IndexSet};
use wsp_engine::link_backend::MachineLink;
use wsp_engine::machine::MachineBackend;
use wsp_wire::views::place::PlaceViewForks;
use wsp_wire::views::place_more::PlaceView;
use wsp_wire::wire::daemon_events::DaemonEvent;
use wsp_wire::wire::machine_link::BackendFacts;
use wsp_wire::wire::place_link::PlaceProveRequest;

use super::context::PlaceDoorContext;
use super::helpers::PendingRecord;
use super::link::{Joining, LinkViews};
use super::records::PlaceRecordsArea;
use super::types::{FolderComputer, PlaceLogin, PlaceRecord, PlaceResult, Proved};

/// What views and manage read off `ctx.door` (views.ts:126,178,236-238,410): places-a's link reads. The door
/// assembly hands a forwarder over `ctx.door()`; a test hands a fake.
#[async_trait(?Send)]
pub trait LinkReads {
    fn backend_of(&self, place_id: &str) -> Option<Arc<dyn MachineBackend>>;
    async fn forking_backend(&self, place_id: &str) -> PlaceResult<Arc<dyn MachineBackend>>;
    fn folder_computer(&self, place_id: &str) -> Option<FolderComputer>;
}

/// What views reads off places-a's setup area (views.ts:44).
pub trait ViewsSetup {
    /// `woken(placeId, back)` (setup.ts:131): wakes every wait on that computer's link.
    fn woken(&self, place_id: &str, back: bool);
    /// `closedAt.delete(placeId)` (setup.ts:128).
    fn forget_closed_at(&self, place_id: &str);
    /// `linkTo(placeId)` (setup.ts): the link a `PlaceMachine` or a `LinkBackend` rides.
    fn link_to(&self, place_id: &str) -> Arc<dyn MachineLink>;
}

/// What a remove took off over the link for the servers wsp merged (V7).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Unmerged {
    pub took: Vec<String>,
    pub kept: Vec<String>,
}

/// What `plugins_off` took off and kept (V10).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PluginsOff {
    pub off: Vec<String>,
    pub kept: Vec<String>,
}

/// The views area (views.ts:38-430).
pub struct PlaceViewsArea {
    pub ctx: Rc<PlaceDoorContext>,
    pub records: Rc<PlaceRecordsArea>,
    pub setup: Rc<dyn ViewsSetup>,
    pub links: Rc<dyn LinkReads>,
    /// V5: the pending adds whose floor runs now.
    pub flooring: Rc<RefCell<IndexSet<String>>>,
    /// V17: what each join told this host before its prove.
    pub joining: Rc<RefCell<IndexMap<String, Joining>>>,
}

impl PlaceViewsArea {
    pub fn new(ctx: Rc<PlaceDoorContext>, records: Rc<PlaceRecordsArea>, setup: Rc<dyn ViewsSetup>, links: Rc<dyn LinkReads>) -> Rc<Self> {
        todo!()
    }

    /// V1 (:48-54).
    pub async fn pending_records(&self) -> PlaceResult<Vec<PendingRecord>> {
        todo!()
    }

    /// V2 (:55-58).
    pub async fn put_pending(&self, p: &PendingRecord) -> PlaceResult<()> {
        todo!()
    }

    /// V3 (:59-62).
    pub async fn drop_pending(&self, id: &str) -> PlaceResult<()> {
        todo!()
    }

    /// V4 (:66-83).
    pub async fn floor_while_choosing(&self, pending: &PendingRecord, place_id: &str) -> PlaceResult<()> {
        todo!()
    }

    /// V5 (:86-94).
    pub async fn floor_once(&self, pending: &PendingRecord, place_id: &str) -> PlaceResult<()> {
        todo!()
    }

    /// V6 (:98-108).
    pub async fn taken_back(&self, pending: &PendingRecord) -> PlaceResult<()> {
        todo!()
    }

    /// V7 (:114-121).
    pub async fn unmerged_over(&self, place_id: &str, held: &PlaceRecord) -> Unmerged {
        todo!()
    }

    /// V8 (:124-128).
    pub async fn stores_over(&self, place_id: &str, home: &str) -> PlaceResult<IndexMap<String, String>> {
        todo!()
    }

    /// V9 (:133-144).
    pub async fn store_files_off(&self, place_id: &str, held: &PlaceRecord) -> Vec<String> {
        todo!()
    }

    /// V10 (:150-171).
    pub async fn plugins_off(&self, place_id: &str, held: &PlaceRecord, linked: bool, login: Option<&PlaceLogin>, sudo_password: Option<&str>) -> PluginsOff {
        todo!()
    }

    /// V11 (:177-189).
    pub async fn facts_on(&self, place_id: &str, held: &PlaceRecord) -> PlaceRecord {
        todo!()
    }

    /// V12 (:193-197).
    pub fn backend_from(&self, place_id: &str, facts: BackendFacts) -> Arc<dyn MachineBackend> {
        todo!()
    }

    /// V13 (:201-216).
    pub fn tunnelled(&self, place_id: &str, event: &DaemonEvent) -> bool {
        todo!()
    }

    /// V14 (:220-228).
    pub fn cut(&self, place_id: &str, reason: &str) {
        todo!()
    }

    /// V15 (:234-256).
    pub async fn forks_of(&self, record: &PlaceRecord) -> Option<PlaceViewForks> {
        todo!()
    }

    /// V16 (:258-314).
    pub fn view_of(&self, record: &PlaceRecord, defaulted: Option<&str>) -> PlaceView {
        todo!()
    }

    /// V17 (:323-357).
    pub async fn joined(&self, place_id: &str, pending: Joining, req: PlaceProveRequest, expect: &[u8], from: &str, at: u64) -> PlaceResult<Proved> {
        todo!()
    }

    /// V18 (:360-375).
    pub async fn forget(&self, place_id: &str) -> PlaceResult<()> {
        todo!()
    }

    /// V19 (:378-401).
    pub fn here_row(&self, marked: &str) -> PlaceView {
        todo!()
    }

    /// V20 (:404-408).
    pub async fn rows_of(&self, held: &[PlaceRecord]) -> PlaceResult<Vec<PlaceView>> {
        todo!()
    }

    /// V21 (:410).
    pub fn joined_row(&self, record: &PlaceRecord, marked: &str) -> PlaceView {
        todo!()
    }

    /// V22 (:411-424).
    pub fn provider_row(&self, id: &str, marked: &str) -> PlaceView {
        todo!()
    }
}

/// places-a's link reads off this area (link.rs `LinkViews`): each a forward to the method above of the same name.
#[async_trait(?Send)]
impl LinkViews for PlaceViewsArea {
    async fn pending_records(&self) -> PlaceResult<Vec<PendingRecord>> {
        todo!()
    }
    async fn put_pending(&self, p: PendingRecord) -> PlaceResult<()> {
        todo!()
    }
    fn flooring(&self) -> Rc<RefCell<IndexSet<String>>> {
        todo!()
    }
    async fn floor_once(&self, pending: PendingRecord, place_id: &str) {
        todo!()
    }
    async fn taken_back(&self, pending: PendingRecord) {
        todo!()
    }
    fn backend_from(&self, place_id: &str, facts: BackendFacts) -> Arc<dyn MachineBackend> {
        todo!()
    }
    fn tunnelled(&self, place_id: &str, e: &DaemonEvent) -> bool {
        todo!()
    }
    fn cut(&self, place_id: &str, reason: &str) {
        todo!()
    }
    fn joining(&self) -> Rc<RefCell<IndexMap<String, Joining>>> {
        todo!()
    }
    async fn joined(&self, place_id: &str, pending: Joining, req: PlaceProveRequest, expect: &[u8], from: &str, at: u64) -> PlaceResult<Proved> {
        todo!()
    }
    async fn joined_row(&self, record: &PlaceRecord, marked: &str) -> PlaceResult<PlaceView> {
        todo!()
    }
}
