// SPDX-License-Identifier: AGPL-3.0-only
//! Port of packages/engine/src/no-provider-backend.ts. Spec: spec/machine.md.
//!
//! The backend a host wires when this computer has no machine provider key: every capability false, an empty
//! listing, and every other road refused with one sentence, typed as [`EngineError::NoProvider`].
use std::sync::Arc;

use async_trait::async_trait;
use indexmap::IndexMap;

use crate::errors::{EngineError, Result};
use crate::machine::{
    BackendPricing, Capabilities, Machine, MachineBackend, MachineListRow, MachineSpec, Size, SnapshotStoragePricing, Waiting,
};

/// no-provider-backend.ts:29. Whether the error is the refusal of a host with no provider key.
pub fn is_no_provider(e: &EngineError) -> bool {
    todo!()
}

/// no-provider-backend.ts:16. Rate 0 for every size, default size 0 cpu and 0 MB, storage free and billed from "".
pub struct NoProviderPricing;

impl BackendPricing for NoProviderPricing {
    fn rate_usd_per_hour(&self, _size: Size) -> f64 {
        todo!()
    }
    fn default_size(&self) -> Size {
        todo!()
    }
    fn snapshot_storage(&self) -> SnapshotStoragePricing {
        todo!()
    }
}

/// no-provider-backend.ts:35.
pub struct NoProviderBackend {
    /// The sentence every road refuses with; `NO_PROVIDER_LINE` where the host names none.
    line: String,
    capabilities: Capabilities,
    pricing: Arc<NoProviderPricing>,
}

impl NoProviderBackend {
    /// `line`: `None` takes wsp-words' `NO_PROVIDER_LINE`.
    pub fn new(line: Option<String>) -> Self {
        todo!()
    }
}

#[async_trait(?Send)]
impl MachineBackend for NoProviderBackend {
    fn capabilities(&self) -> Capabilities {
        self.capabilities.clone()
    }
    fn pricing(&self) -> Arc<dyn BackendPricing> {
        self.pricing.clone()
    }
    async fn create(&self, _spec: MachineSpec, _waiting: Option<Waiting>, _sent: Option<String>) -> Result<Arc<dyn Machine>> {
        todo!()
    }
    async fn get(&self, _id: &str) -> Result<Arc<dyn Machine>> {
        todo!()
    }
    async fn list(&self, _labels: Option<&IndexMap<String, String>>) -> Result<Vec<MachineListRow>> {
        todo!()
    }
    async fn delete_snapshot(&self, _id: &str) -> Result<()> {
        todo!()
    }
}
