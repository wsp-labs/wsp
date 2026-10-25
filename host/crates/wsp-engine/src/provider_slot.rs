// SPDX-License-Identifier: AGPL-3.0-only
//! Port of packages/engine/src/provider-slot.ts. Spec: spec/small.md.
//!
//! One backend the runtime holds for the life of a host, standing in front of whichever provider module is current.
use std::sync::{Arc, RwLock};

use async_trait::async_trait;
use indexmap::IndexMap;

use crate::errors::Result;
use crate::machine::{BackendPricing, Capabilities, Machine, MachineBackend, MachineListRow, MachineSpec, Waiting};

/// provider-slot.ts:9. Every read and call lands on the module current at that moment.
pub struct ProviderSlot {
    target: RwLock<Arc<dyn MachineBackend>>,
}

impl ProviderSlot {
    /// provider-slot.ts:16.
    pub fn new(initial: Arc<dyn MachineBackend>) -> Arc<Self> {
        todo!()
    }

    /// What the runtime is wired with: the slot itself as a backend.
    pub fn backend(self: &Arc<Self>) -> Arc<dyn MachineBackend> {
        todo!()
    }

    pub fn swap(&self, next: Arc<dyn MachineBackend>) {
        todo!()
    }

    /// The module current now: the very `Arc` last handed to `new` or `swap`.
    pub fn current(&self) -> Arc<dyn MachineBackend> {
        todo!()
    }
}

/// Forwards every method, the optional ones and `offers()` included, to `current()` at the moment of the call.
#[async_trait(?Send)]
impl MachineBackend for ProviderSlot {
    fn capabilities(&self) -> Capabilities {
        todo!()
    }
    fn pricing(&self) -> Arc<dyn BackendPricing> {
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
}
