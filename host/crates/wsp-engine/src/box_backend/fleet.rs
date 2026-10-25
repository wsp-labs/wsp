// SPDX-License-Identifier: AGPL-3.0-only
//! box-backend.ts:788-1044 at a4033dc34. Spec: spec/box_fleet.md.
use std::collections::BTreeSet;
use std::sync::Arc;

use async_trait::async_trait;
use indexmap::IndexMap;
use serde_json::Value;

use super::machine::BoxMachine;
use super::plan::BoxLimits;
use super::request::{BoxBackend, BoxView, NamedSnapshotView};
use crate::errors::Result;
use crate::machine::{
    BackendOffers, BackendPricing, Capabilities, CloudLimits, Lifecycle, LifecycleBudgets, Machine, MachineBackend, MachineKind,
    MachineListRow, MachineSpec, NamedMachine, RootDisk, Size, SnapshotRow, TemplateRow, Waiting,
};

/// What `postCreate` answers: the create's answer and, where one was lost on the way, what was said then.
#[derive(Debug, Clone, PartialEq)]
pub struct PostedCreate {
    /// The answer's `status`, where it carried one.
    pub status: Option<String>,
    pub sandbox: BoxView,
    pub lost: Option<String>,
}

impl BoxBackend {
    /// box-backend.ts:543. The classes the plan sells off the last read of /limits; every class before the first.
    pub fn box_capabilities(&self) -> Capabilities {
        todo!()
    }

    /// box-backend.ts:790. The exact bytes a create of this spec sends.
    pub async fn box_create_request(&self, spec: &MachineSpec) -> Result<String> {
        todo!()
    }

    /// box-backend.ts:810.
    pub async fn post_create(&self, key: &str, bytes: &str) -> Result<PostedCreate> {
        todo!()
    }

    /// box-backend.ts:831.
    pub async fn box_create(&self, spec: MachineSpec, waiting: Option<Waiting>, sent: Option<String>) -> Result<BoxMachine> {
        todo!()
    }

    /// box-backend.ts:880. `made_at` is an ISO date as `Date.parse` reads it.
    pub async fn box_replay_create(&self, key: &str, bytes: &str, made_at: &str) -> Result<Option<BoxMachine>> {
        todo!()
    }

    /// box-backend.ts:894.
    pub async fn box_named_after(&self, names: &BTreeSet<String>) -> Result<Vec<NamedMachine>> {
        todo!()
    }

    /// box-backend.ts:903.
    pub async fn box_get(&self, id: &str) -> Result<BoxMachine> {
        todo!()
    }

    /// box-backend.ts:917. Every row of a cursor-paged listing, 200 a page, until no cursor or `enough` says so.
    pub async fn pages(
        &self,
        path: &str,
        rows_key: &str,
        query: &[(&str, &str)],
        enough: Option<&(dyn Fn(&[Value]) -> bool)>,
    ) -> Result<Vec<Value>> {
        todo!()
    }

    /// box-backend.ts:932. The plain listing then the archived one, duplicates kept.
    pub async fn every_box(&self) -> Result<Vec<BoxView>> {
        todo!()
    }

    /// box-backend.ts:940.
    pub async fn box_list(&self, labels: Option<&IndexMap<String, String>>) -> Result<Vec<MachineListRow>> {
        todo!()
    }

    /// box-backend.ts:965.
    pub async fn box_list_snapshots(&self, ids: Option<&[String]>) -> Result<Vec<SnapshotRow>> {
        todo!()
    }

    /// box-backend.ts:982.
    pub async fn box_delete_snapshot(&self, id: &str) -> Result<()> {
        todo!()
    }

    /// box-backend.ts:993.
    pub async fn named_snapshots(&self) -> Result<Vec<NamedSnapshotView>> {
        todo!()
    }

    /// box-backend.ts:1004.
    pub async fn box_get_template(&self, id: &str) -> Result<TemplateRow> {
        todo!()
    }

    /// box-backend.ts:1008.
    pub async fn box_list_templates(&self) -> Result<Vec<TemplateRow>> {
        todo!()
    }

    /// box-backend.ts:1019. `until` in ms since the epoch.
    pub async fn box_backstop(&self, machine_id: &str, until: i64, exact: bool) -> Result<()> {
        todo!()
    }

    /// box-backend.ts:1040.
    pub fn forget_backstop(&self, id: &str) {
        todo!()
    }
}

/// box-backend.ts:557. Budgets [`super::plan::box_budgets`], the backstop through the backend, disk kept 14 days.
pub struct BoxLifecycle {
    pub backend: BoxBackend,
}

#[async_trait(?Send)]
impl Lifecycle for BoxLifecycle {
    fn budgets(&self) -> LifecycleBudgets {
        todo!()
    }
    fn has_backstop(&self) -> bool {
        todo!()
    }
    async fn backstop(&self, machine: &dyn Machine, until_ms: i64, exact: bool) -> Result<()> {
        todo!()
    }
    fn disk_kept_ms(&self) -> Option<u64> {
        todo!()
    }
}

#[async_trait(?Send)]
impl MachineBackend for BoxBackend {
    fn capabilities(&self) -> Capabilities {
        todo!()
    }
    fn pricing(&self) -> Arc<dyn BackendPricing> {
        todo!()
    }
    fn lifecycle(&self) -> Option<Arc<dyn Lifecycle>> {
        todo!()
    }
    /// sandbox and desktop both boot `base`.
    fn base_templates(&self) -> Option<crate::machine::BaseTemplates> {
        todo!()
    }
    /// keyed_create, check_key, place_limits, root_disk_of, limits, list_snapshots, templates.
    fn offers(&self) -> BackendOffers {
        todo!()
    }
    /// box-backend.ts:551.
    fn root_disk_of(&self, size: Size) -> Option<RootDisk> {
        todo!()
    }
    async fn create(&self, spec: MachineSpec, waiting: Option<Waiting>, sent: Option<String>) -> Result<Arc<dyn Machine>> {
        todo!()
    }
    async fn get(&self, id: &str) -> Result<Arc<dyn Machine>> {
        todo!()
    }
    async fn list(&self, labels: Option<&IndexMap<String, String>>) -> Result<Vec<MachineListRow>> {
        todo!()
    }
    async fn delete_snapshot(&self, id: &str) -> Result<()> {
        todo!()
    }
    async fn create_request(&self, spec: &MachineSpec) -> Result<String> {
        todo!()
    }
    async fn replay_create(&self, key: &str, bytes: &str, made_at: &str) -> Result<Option<Arc<dyn Machine>>> {
        todo!()
    }
    async fn named_after(&self, names: &BTreeSet<String>) -> Result<Vec<NamedMachine>> {
        todo!()
    }
    /// box-backend.ts:747: the account's limits, as JSON.
    async fn check_key(&self) -> Result<Value> {
        todo!()
    }
    async fn place_limits(&self) -> Result<Option<CloudLimits>> {
        todo!()
    }
    async fn limits(&self) -> Result<BoxLimits> {
        todo!()
    }
    async fn list_snapshots(&self, ids: Option<&[String]>) -> Result<Vec<SnapshotRow>> {
        todo!()
    }
    /// box-backend.ts:1000: the template is the name it was saved under.
    async fn promote_snapshot(&self, snapshot_id: &str, name: &str) -> Result<String> {
        todo!()
    }
    async fn get_template(&self, id: &str) -> Result<TemplateRow> {
        todo!()
    }
    async fn list_templates(&self) -> Result<Vec<TemplateRow>> {
        todo!()
    }
    async fn delete_template(&self, id: &str) -> Result<()> {
        todo!()
    }
}
