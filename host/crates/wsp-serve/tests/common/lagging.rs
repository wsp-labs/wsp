// SPDX-License-Identifier: AGPL-3.0-only
use std::cell::RefCell;
use std::sync::Arc;

use super::stub_backend::StubBackend;
use wsp_engine::machine::MachineBackend;

/// `backend.list = async labels => (await list(labels)).filter(r => r.id !== mb.id && r.id !== mc.id)`: a provider whose
/// listing lags, over the stub, which answers every other call itself.
pub struct Lagging {
    inner: Arc<StubBackend>,
    hidden: RefCell<Vec<String>>,
}

impl Lagging {
    pub fn over(inner: &Arc<StubBackend>) -> Arc<Lagging> {
        Arc::new(Lagging { inner: inner.clone(), hidden: RefCell::default() })
    }
    pub fn hide(&self, ids: &[String]) {
        self.hidden.borrow_mut().extend(ids.iter().cloned());
    }
}

mod backend {
    use super::*;
    use async_trait::async_trait;
    use indexmap::IndexMap;
    use std::collections::BTreeSet;
    use wsp_engine::errors::Result as R;
    use wsp_engine::machine::{
        BackendOffers, BackendPricing, Capabilities, ExecResult, Lifecycle, Machine, MachineListRow, MachineSpec, NamedMachine, RootDisk,
        Size, SnapshotRow, TemplateRow, Waiting,
    };

    #[async_trait(?Send)]
    impl MachineBackend for Lagging {
        fn capabilities(&self) -> Capabilities {
            self.inner.capabilities()
        }
        fn pricing(&self) -> Arc<dyn BackendPricing> {
            self.inner.pricing()
        }
        fn lifecycle(&self) -> Option<Arc<dyn Lifecycle>> {
            MachineBackend::lifecycle(&*self.inner)
        }
        fn names_workspace(&self) -> bool {
            self.inner.names_workspace()
        }
        fn offers(&self) -> BackendOffers {
            self.inner.offers()
        }
        fn root_disk_of(&self, size: Size) -> Option<RootDisk> {
            self.inner.root_disk_of(size)
        }
        async fn create(&self, spec: MachineSpec, waiting: Option<Waiting>, sent: Option<String>) -> R<Arc<dyn Machine>> {
            self.inner.create(spec, waiting, sent).await
        }
        async fn get(&self, id: &str) -> R<Arc<dyn Machine>> {
            self.inner.get(id).await
        }
        async fn list(&self, labels: Option<&IndexMap<String, String>>) -> R<Vec<MachineListRow>> {
            let hidden = self.hidden.borrow().clone();
            Ok(self.inner.list(labels).await?.into_iter().filter(|r| !hidden.contains(&r.id)).collect())
        }
        async fn delete_snapshot(&self, id: &str) -> R<()> {
            self.inner.delete_snapshot(id).await
        }
        async fn on_computer(&self, cmd: &str, timeout_ms: Option<u64>) -> R<ExecResult> {
            self.inner.on_computer(cmd, timeout_ms).await
        }
        async fn create_request(&self, spec: &MachineSpec) -> R<String> {
            self.inner.create_request(spec).await
        }
        async fn replay_create(&self, key: &str, bytes: &str, made_at: &str) -> R<Option<Arc<dyn Machine>>> {
            self.inner.replay_create(key, bytes, made_at).await
        }
        async fn named_after(&self, names: &BTreeSet<String>) -> R<Vec<NamedMachine>> {
            self.inner.named_after(names).await
        }
        async fn list_snapshots(&self, ids: Option<&[String]>) -> R<Vec<SnapshotRow>> {
            self.inner.list_snapshots(ids).await
        }
        async fn promote_snapshot(&self, snapshot_id: &str, name: &str) -> R<String> {
            self.inner.promote_snapshot(snapshot_id, name).await
        }
        async fn get_template(&self, id: &str) -> R<TemplateRow> {
            self.inner.get_template(id).await
        }
        async fn list_templates(&self) -> R<Vec<TemplateRow>> {
            self.inner.list_templates().await
        }
        async fn delete_template(&self, id: &str) -> R<()> {
            self.inner.delete_template(id).await
        }
    }
}
