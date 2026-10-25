// SPDX-License-Identifier: AGPL-3.0-only
//! Port of packages/engine/src/provider-slot.ts. Spec: spec/small.md.
//!
//! One backend the runtime holds for the life of a host, standing in front of whichever provider module is current.
use std::collections::BTreeSet;
use std::sync::{Arc, RwLock};

use async_trait::async_trait;
use indexmap::IndexMap;
use serde_json::Value;

use crate::box_backend::BoxLimits;
use crate::errors::Result;
use crate::machine::{
    BackendOffers, BackendPricing, BaseTemplates, Capabilities, CloudLimits, ExecResult, Lifecycle, Machine, MachineBackend,
    MachineListRow, MachineSpec, NamedMachine, PlaceCapacity, RootDisk, Size, SnapshotRow, TemplateRow, Waiting,
};

/// provider-slot.ts:9. Every read and call lands on the module current at that moment.
pub struct ProviderSlot {
    target: RwLock<Arc<dyn MachineBackend>>,
}

impl ProviderSlot {
    /// provider-slot.ts:16.
    pub fn new(initial: Arc<dyn MachineBackend>) -> Arc<Self> {
        Arc::new(ProviderSlot { target: RwLock::new(initial) })
    }

    /// What the runtime is wired with: the slot itself as a backend.
    pub fn backend(self: &Arc<Self>) -> Arc<dyn MachineBackend> {
        self.clone()
    }

    pub fn swap(&self, next: Arc<dyn MachineBackend>) {
        *self.target.write().unwrap() = next;
    }

    /// The module current now: the very `Arc` last handed to `new` or `swap`.
    pub fn current(&self) -> Arc<dyn MachineBackend> {
        self.target.read().unwrap().clone()
    }
}

/// Forwards every method, the optional ones and `offers()` included, to `current()` at the moment of the call.
#[async_trait(?Send)]
impl MachineBackend for ProviderSlot {
    fn capabilities(&self) -> Capabilities {
        self.current().capabilities()
    }
    fn pricing(&self) -> Arc<dyn BackendPricing> {
        self.current().pricing()
    }
    fn lifecycle(&self) -> Option<Arc<dyn Lifecycle>> {
        self.current().lifecycle()
    }
    fn base_templates(&self) -> Option<BaseTemplates> {
        self.current().base_templates()
    }
    fn names_workspace(&self) -> bool {
        self.current().names_workspace()
    }
    fn logins(&self) -> Option<String> {
        self.current().logins()
    }
    fn projects(&self) -> Option<String> {
        self.current().projects()
    }
    fn offers(&self) -> BackendOffers {
        self.current().offers()
    }
    fn root_disk_of(&self, size: Size) -> Option<RootDisk> {
        self.current().root_disk_of(size)
    }
    async fn create(&self, spec: MachineSpec, waiting: Option<Waiting>, sent: Option<String>) -> Result<Arc<dyn Machine>> {
        self.current().create(spec, waiting, sent).await
    }
    async fn get(&self, id: &str) -> Result<Arc<dyn Machine>> {
        self.current().get(id).await
    }
    async fn list(&self, labels: Option<&IndexMap<String, String>>) -> Result<Vec<MachineListRow>> {
        self.current().list(labels).await
    }
    async fn delete_snapshot(&self, id: &str) -> Result<()> {
        self.current().delete_snapshot(id).await
    }
    async fn on_computer(&self, cmd: &str, timeout_ms: Option<u64>) -> Result<ExecResult> {
        self.current().on_computer(cmd, timeout_ms).await
    }
    async fn create_request(&self, spec: &MachineSpec) -> Result<String> {
        self.current().create_request(spec).await
    }
    async fn replay_create(&self, key: &str, bytes: &str, made_at: &str) -> Result<Option<Arc<dyn Machine>>> {
        self.current().replay_create(key, bytes, made_at).await
    }
    async fn named_after(&self, names: &BTreeSet<String>) -> Result<Vec<NamedMachine>> {
        self.current().named_after(names).await
    }
    async fn check_key(&self) -> Result<Value> {
        self.current().check_key().await
    }
    async fn place_limits(&self) -> Result<Option<CloudLimits>> {
        self.current().place_limits().await
    }
    async fn capacity(&self) -> Result<PlaceCapacity> {
        self.current().capacity().await
    }
    async fn limits(&self) -> Result<BoxLimits> {
        self.current().limits().await
    }
    async fn list_snapshots(&self, ids: Option<&[String]>) -> Result<Vec<SnapshotRow>> {
        self.current().list_snapshots(ids).await
    }
    async fn promote_snapshot(&self, snapshot_id: &str, name: &str) -> Result<String> {
        self.current().promote_snapshot(snapshot_id, name).await
    }
    async fn get_template(&self, id: &str) -> Result<TemplateRow> {
        self.current().get_template(id).await
    }
    async fn list_templates(&self) -> Result<Vec<TemplateRow>> {
        self.current().list_templates().await
    }
    async fn delete_template(&self, id: &str) -> Result<()> {
        self.current().delete_template(id).await
    }
}
