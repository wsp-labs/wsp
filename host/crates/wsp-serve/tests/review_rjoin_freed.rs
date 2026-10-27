// SPDX-License-Identifier: AGPL-3.0-only
//! Review of join A (#2476), must-fix 5, under a counting allocator: a runtime that booted, closed and was dropped
//! gives back what it took, so the bytes still held do not grow with the number of runtimes made.
use std::alloc::{GlobalAlloc, Layout, System};
use std::rc::Rc;
use std::sync::atomic::{AtomicIsize, Ordering};
use std::sync::Arc;

use wsp_core::clock::FakeClock;
use wsp_core::types::api::Runtime;
use wsp_core::types::wiring::RuntimeOptions;
use wsp_engine::fake_backend::{FakeBackend, FakeBackendOptions};
use wsp_engine::machine::MachineBackend;
use wsp_serve::create_runtime;
use wsp_store::sqlite_store::sqlite_store;
use wsp_store::store::{StateWriter, Store};
use wsp_wire::types::js_number::JsInt;

struct Counting;

static LIVE: AtomicIsize = AtomicIsize::new(0);

unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        LIVE.fetch_add(layout.size() as isize, Ordering::Relaxed);
        System.alloc(layout)
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        LIVE.fetch_sub(layout.size() as isize, Ordering::Relaxed);
        System.dealloc(ptr, layout)
    }
    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        LIVE.fetch_add(new_size as isize - layout.size() as isize, Ordering::Relaxed);
        System.realloc(ptr, layout, new_size)
    }
}

#[global_allocator]
static ALLOC: Counting = Counting;

async fn one_runtime(home: &std::path::Path) {
    let writer = StateWriter { wsp: "0.0.0".into(), daemon: JsInt::from(1), bin: "/bin/wsp".into() };
    let store: Rc<dyn Store> = Rc::new(sqlite_store(&home.join("state.json"), writer));
    let backend: Arc<dyn MachineBackend> = Arc::new(FakeBackend::new(FakeBackendOptions::default()));
    let clock = FakeClock::new(1_000_000);
    let opts = RuntimeOptions { backend: Some(backend), store: Some(store), clock: Some(clock.clock()), ..Default::default() };
    let rt = create_runtime(opts).expect("a runtime");
    let _ = rt.workspaces().list(None).await;
    rt.close().await.expect("close");
    drop(rt);
    for _ in 0..50 {
        tokio::task::yield_now().await;
    }
}

#[test]
fn a_closed_and_dropped_runtime_gives_back_its_memory() {
    let tokio_rt = tokio::runtime::Builder::new_current_thread().enable_all().build().expect("a tokio runtime");
    let local = tokio::task::LocalSet::new();
    let held = local.block_on(&tokio_rt, async {
        let home = tempfile::tempdir().expect("a temp folder");
        for _ in 0..3 {
            one_runtime(home.path()).await;
        }
        let before = LIVE.load(Ordering::Relaxed);
        for _ in 0..20 {
            one_runtime(home.path()).await;
        }
        LIVE.load(Ordering::Relaxed) - before
    });
    assert!(held < 64 * 1024, "20 runtimes made, closed and dropped still hold {held} bytes ({} each)", held / 20);
}
