// SPDX-License-Identifier: AGPL-3.0-only
//! A backend the link tests hand out where nothing calls it, and what a daemon says about the one it forks with
//! (`test/places-setup.test.ts:101` FACTS).

use std::sync::Arc;

use async_trait::async_trait;
use indexmap::IndexMap;
use serde_json::{json, Map, Value};
use wsp_engine::errors::Result as EngineResult;
use wsp_engine::machine::{BackendPricing, Capabilities, Machine, MachineBackend, MachineListRow, MachineSpec, Size, SnapshotStoragePricing, Waiting, WireSnapshotStoragePricing};

use super::places::obj;

struct Free;

impl BackendPricing for Free {
    fn rate_usd_per_hour(&self, _size: Size) -> f64 {
        0.0
    }
    fn default_size(&self) -> Size {
        Size { cpu: 2.0, mem_mb: 4096.0 }
    }
    fn snapshot_storage(&self) -> SnapshotStoragePricing {
        let wire: WireSnapshotStoragePricing = serde_json::from_value(json!({ "freeGb": 0, "usdPerGbMonth": 0, "billedFrom": "" })).expect("a pricing");
        SnapshotStoragePricing { wire, free_named: None, usd_per_named_month: None }
    }
}

/// A backend tagged with a name, so a test tells which one it was handed.
pub struct Tagged {
    pub tag: String,
}

#[async_trait(?Send)]
impl MachineBackend for Tagged {
    fn capabilities(&self) -> Capabilities {
        serde_json::from_value(facts_json(None)["capabilities"].clone()).expect("capabilities")
    }
    fn pricing(&self) -> Arc<dyn BackendPricing> {
        Arc::new(Free)
    }
    fn logins(&self) -> Option<String> {
        Some(self.tag.clone())
    }
    async fn create(&self, _spec: MachineSpec, _waiting: Option<Waiting>, _sent: Option<String>) -> EngineResult<Arc<dyn Machine>> {
        unreachable!("nothing is created here")
    }
    async fn get(&self, _id: &str) -> EngineResult<Arc<dyn Machine>> {
        unreachable!("nothing is read here")
    }
    async fn list(&self, _labels: Option<&IndexMap<String, String>>) -> EngineResult<Vec<MachineListRow>> {
        Ok(Vec::new())
    }
    async fn delete_snapshot(&self, _id: &str) -> EngineResult<()> {
        Ok(())
    }
}

pub fn tagged(tag: &str) -> Arc<dyn MachineBackend> {
    Arc::new(Tagged { tag: tag.to_string() })
}

/// What a computer that keeps no image says it forks with, with its logins folder where given.
pub fn facts_json(logins: Option<&str>) -> Value {
    let mut v = json!({
        "offer": "docker",
        "capabilities": {
            "liveCloneForks": false, "pauseMode": "memory", "replacesMachine": true, "previewUrls": false, "signedUrls": false,
            "callbackRelay": true, "diskSnapshots": false, "images": false, "snapshotsAnyLife": false, "snapshotListing": false,
            "templates": false, "kept": false, "copies": true, "ownNetwork": true,
            "sizes": [{ "cpu": 2, "memMb": 4096, "rateUsdPerHour": 0 }],
        },
        "pricing": { "defaultSize": { "cpu": 2, "memMb": 4096 }, "snapshotStorage": { "freeGb": 0, "usdPerGbMonth": 0, "billedFrom": "" } },
        "lifecycle": { "budgets": { "wakeAttempts": 1, "daemonAnswersMs": 30000 } },
        "baseTemplates": { "sandbox": "ubuntu:24.04", "desktop": "ubuntu:24.04" },
    });
    if let Some(l) = logins {
        v["logins"] = json!(l);
    }
    v
}

/// The `machine.backend` answer as the daemon sends it.
pub fn said_facts(logins: Option<&str>) -> Map<String, Value> {
    let mut m = obj(facts_json(logins));
    m.insert("ok".into(), json!(true));
    m
}
