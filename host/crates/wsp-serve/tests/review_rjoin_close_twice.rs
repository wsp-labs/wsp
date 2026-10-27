// SPDX-License-Identifier: AGPL-3.0-only
//! Review of join A (#2476), must-fix 5: the close now empties every area slot, and a second close in node resolves
//! (`await rt.close(); await rt.close()` on createRuntime at 65f6b89c7, runtime.ts:1217), so it must not read an emptied
//! slot and panic.
use std::rc::Rc;
use std::sync::Arc;

use wsp_core::types::api::Runtime;
use wsp_core::types::wiring::RuntimeOptions;
use wsp_engine::fake_backend::{FakeBackend, FakeBackendOptions};
use wsp_engine::machine::MachineBackend;
use wsp_serve::create_runtime;
use wsp_store::store::{memory_store, Store};

#[test]
fn a_second_close_answers_as_the_first_did() {
    let tokio_rt = tokio::runtime::Builder::new_current_thread().enable_all().build().expect("a tokio runtime");
    tokio::task::LocalSet::new().block_on(&tokio_rt, async {
        let backend: Arc<dyn MachineBackend> = Arc::new(FakeBackend::new(FakeBackendOptions::default()));
        let store: Rc<dyn Store> = Rc::new(memory_store());
        let rt = create_runtime(RuntimeOptions { backend: Some(backend), store: Some(store), ..Default::default() }).expect("a runtime");
        rt.close().await.expect("the first close");
        let again = futures::FutureExt::catch_unwind(std::panic::AssertUnwindSafe(rt.close())).await;
        assert!(matches!(again, Ok(Ok(()))), "the second close panicked or failed where node's resolves");
    });
}
