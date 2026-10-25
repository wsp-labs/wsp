// SPDX-License-Identifier: AGPL-3.0-only
//! Hard tests of the seam and the no-provider backend (spec/machine.md).
#[path = "oracle/mod.rs"]
mod oracle;

use std::sync::Arc;

use async_trait::async_trait;
use indexmap::IndexMap;
use wsp_engine::errors::{EngineError, Result};
use wsp_engine::machine::*;
use wsp_engine::no_provider_backend::{is_no_provider, NoProviderBackend};

#[test]
fn no_provider_capabilities_are_the_ts_object_in_its_key_order() {
    let o = oracle::load("no_provider.json");
    let got = serde_json::to_value(NoProviderBackend::new(None).capabilities()).unwrap();
    assert_eq!(got, o["capabilities"]);
    let keys: Vec<&str> = got.as_object().unwrap().keys().map(String::as_str).collect();
    let want: Vec<&str> = o["capabilitiesKeys"].as_array().unwrap().iter().map(oracle::s).collect();
    assert_eq!(keys, want);
    let st = NoProviderBackend::new(None).pricing().snapshot_storage();
    assert_eq!(serde_json::to_value(&st.wire).unwrap(), o["snapshotStorage"]);
    assert_eq!((st.free_named, st.usd_per_named_month), (None, None));
}

#[tokio::test]
async fn the_refusal_is_typed_so_a_record_waits_and_a_host_line_replaces_the_default() {
    let b = NoProviderBackend::new(Some("add a provider with wsp add boat".into()));
    let e = b.get("bx_1").await.err().expect("a refusal");
    assert!(is_no_provider(&e));
    assert_eq!(e.to_string(), "add a provider with wsp add boat");
    assert!(matches!(&e, EngineError::NoProvider { line } if line == "add a provider with wsp add boat"));
    let missing = EngineError::Provider(wsp_engine::errors::classify(404, None, None, None));
    assert!(!is_no_provider(&missing));
    assert!(!wsp_engine::errors::is_missing(&e), "a host with no key is not a provider saying gone");
    let mut labels = IndexMap::new();
    labels.insert("wsp".to_string(), "1".to_string());
    assert_eq!(b.list(Some(&labels)).await.unwrap(), vec![]);
}

#[test]
fn no_provider_offers_nothing_optional() {
    let b = NoProviderBackend::new(None);
    assert_eq!(b.offers(), BackendOffers::default());
    assert!(b.lifecycle().is_none());
    assert!(b.base_templates().is_none());
    assert!(!b.names_workspace());
    assert_eq!(b.root_disk_of(Size { cpu: 2.0, mem_mb: 4096.0 }), None);
    assert_eq!(b.pricing().rate_usd_per_hour(Size { cpu: 64.0, mem_mb: 1e6 }), 0.0);
    assert_eq!(b.pricing().default_size(), Size { cpu: 0.0, mem_mb: 0.0 });
    assert_eq!(b.pricing().builder_disk_gb(), None);
}

struct Bare;

#[async_trait]
impl Machine for Bare {
    fn id(&self) -> &str {
        "bare"
    }
    fn kind(&self) -> MachineKind {
        MachineKind::Sandbox
    }
    async fn exec(&self, _: &str, _: ExecOptions) -> Result<ExecResult> {
        unreachable!()
    }
    async fn run(&self, _: &str, _: RunOptions) -> Result<ExecResult> {
        unreachable!()
    }
    async fn snapshot(&self, _: &str, _: MachineLife, _: SnapshotOptions) -> Result<String> {
        unreachable!()
    }
    async fn pause(&self, _: Option<u64>) -> Result<()> {
        unreachable!()
    }
    async fn resume(&self, _: Option<tokio_util::sync::CancellationToken>) -> Result<()> {
        unreachable!()
    }
    async fn kill(&self) -> Result<()> {
        unreachable!()
    }
    async fn state(&self) -> Result<MachineState> {
        unreachable!()
    }
    async fn download_url(&self, _: &str) -> Result<String> {
        unreachable!()
    }
    async fn upload_url(&self, _: &str) -> Result<String> {
        unreachable!()
    }
}

#[tokio::test]
async fn a_machine_without_an_optional_method_answers_not_on_this_road() {
    // The seam's own default bodies, which are written (not todo!): this one passes now and pins them.
    let m: Arc<dyn Machine> = Arc::new(Bare);
    assert_eq!(m.offers(), MachineOffers::default());
    let e = m.preview_url(3000).await.unwrap_err();
    assert_eq!(e.to_string(), wsp_wire::wire::machine_link::NOT_ON_THIS_ROAD);
    assert!(m.put_bytes("/tmp/x", b"x", None).await.is_err());
    assert!(!m.takes_stdin());
    assert!(m.labels().is_none() && m.seen().is_none() && !m.replayed() && m.notice().is_none());
}
