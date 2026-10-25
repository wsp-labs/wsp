// SPDX-License-Identifier: AGPL-3.0-only
//! Port of packages/engine/src/no-provider-backend.ts. Spec: spec/machine.md.
//!
//! The backend a host wires when this computer has no machine provider key: every capability false, an empty
//! listing, and every other road refused with one sentence, typed as [`EngineError::NoProvider`].
use std::sync::Arc;

use async_trait::async_trait;
use indexmap::IndexMap;
use wsp_wire::types::JsNumber;
use wsp_words::words::thread::NO_PROVIDER_LINE;

use crate::errors::{EngineError, Result};
use crate::machine::{
    BackendPricing, Capabilities, Machine, MachineBackend, MachineListRow, MachineSpec, Size, SnapshotStoragePricing, Waiting,
    WireSnapshotStoragePricing,
};

/// no-provider-backend.ts:29. Whether the error is the refusal of a host with no provider key.
pub fn is_no_provider(e: &EngineError) -> bool {
    matches!(e, EngineError::NoProvider { .. })
}

/// no-provider-backend.ts:16. Rate 0 for every size, default size 0 cpu and 0 MB, storage free and billed from "".
pub struct NoProviderPricing;

impl BackendPricing for NoProviderPricing {
    fn rate_usd_per_hour(&self, _size: Size) -> f64 {
        0.0
    }
    fn default_size(&self) -> Size {
        Size { cpu: 0.0, mem_mb: 0.0 }
    }
    fn snapshot_storage(&self) -> SnapshotStoragePricing {
        SnapshotStoragePricing {
            wire: WireSnapshotStoragePricing { free_gb: JsNumber(0.0), usd_per_gb_month: JsNumber(0.0), billed_from: String::new() },
            free_named: None,
            usd_per_named_month: None,
        }
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
        let capabilities = Capabilities {
            live_clone_forks: false,
            pause_mode: None,
            replaces_machine: false,
            preview_urls: false,
            signed_urls: false,
            callback_relay: false,
            disk_snapshots: false,
            images: false,
            snapshots_any_life: false,
            snapshot_listing: false,
            templates: false,
            sizes: vec![],
            kept: false,
            copies: false,
            own_network: false,
            sizes_last: false,
        };
        NoProviderBackend { line: line.unwrap_or_else(|| NO_PROVIDER_LINE.to_string()), capabilities, pricing: Arc::new(NoProviderPricing) }
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
        Err(EngineError::NoProvider { line: self.line.clone() })
    }
    async fn get(&self, _id: &str) -> Result<Arc<dyn Machine>> {
        Err(EngineError::NoProvider { line: self.line.clone() })
    }
    async fn list(&self, _labels: Option<&IndexMap<String, String>>) -> Result<Vec<MachineListRow>> {
        Ok(vec![])
    }
    async fn delete_snapshot(&self, _id: &str) -> Result<()> {
        Err(EngineError::NoProvider { line: self.line.clone() })
    }
}
