// SPDX-License-Identifier: AGPL-3.0-only
//! What `create_runtime` builds beyond the areas: the place door over the host's wiring, and the refusals of a
//! runtime wired with nothing to run on (runtime.ts:493-790).
use std::rc::Rc;
use std::sync::Arc;

use wsp_core::types::api::Runtime;
use wsp_core::types::pending::PlaceWiring as WiringMarker;
use wsp_core::types::wiring::RuntimeOptions;
use wsp_engine::fake_backend::{FakeBackend, FakeBackendOptions};
use wsp_machines::places::types::{HerePlace, PlaceWiring};
use wsp_serve::create_runtime;
use wsp_store::keys::{key_fingerprint, new_place_key_pair};
use wsp_store::store::memory_store;

fn options() -> RuntimeOptions {
    let backend = Arc::new(FakeBackend::new(FakeBackendOptions::default()));
    RuntimeOptions { backend: Some(backend), store: Some(Rc::new(memory_store())), ..Default::default() }
}

fn wiring(host_key: wsp_store::keys::PlaceKeyPair) -> PlaceWiring {
    PlaceWiring {
        host_key,
        provider: None,
        here: Rc::new(|| HerePlace { name: "zingzys-mac".into(), ..Default::default() }),
        host_name: Rc::new(|| "zingzys-mac".to_string()),
        install: None,
        dial: None,
        log: None,
        update: None,
        leave: None,
        forget_key: None,
        sudo_over: None,
        provision: None,
        github_token: None,
        undo: None,
        run_over: None,
        back: None,
    }
}

fn run<Fut: std::future::Future<Output = ()>>(f: impl FnOnce() -> Fut) {
    let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().expect("a tokio runtime");
    let local = tokio::task::LocalSet::new();
    local.block_on(&rt, async move { f().await });
}

#[test]
fn a_runtime_wired_with_place_links_serves_a_place_door_under_the_hosts_key() {
    run(|| async {
        let key = new_place_key_pair();
        let mut opts = options();
        opts.place_links = Some(Rc::new(wiring(key.clone())));
        let runtime = create_runtime(opts);
        let door = runtime.places().expect("the door the links wire");
        assert_eq!(door.host_key(), key_fingerprint(&key.public_key));
    });
}

#[test]
#[should_panic(expected = "opts.place_links is not the machines' PlaceWiring")]
fn place_links_of_another_type_are_refused_where_the_runtime_is_built() {
    struct Other;
    impl WiringMarker for Other {}
    run(|| async {
        let mut opts = options();
        opts.place_links = Some(Rc::new(Other));
        let _ = create_runtime(opts);
    });
}

#[test]
#[should_panic(expected = "create_runtime needs a store")]
fn a_runtime_wired_with_no_store_is_refused() {
    run(|| async {
        let backend = Arc::new(FakeBackend::new(FakeBackendOptions::default()));
        let _ = create_runtime(RuntimeOptions { backend: Some(backend), ..Default::default() });
    });
}
