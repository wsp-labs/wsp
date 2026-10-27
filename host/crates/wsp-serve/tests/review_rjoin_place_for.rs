// SPDX-License-Identifier: AGPL-3.0-only
//! Review of join A (#2476), ruling 4 on a method left plain: `usage.reset` with `on` reads the word as a place first,
//! and `placeFor` throws for a word that names no computer (places/link.ts:518, account/usage.ts:307), so the spend is
//! refused in those words. It must never land on this computer instead.
use std::path::Path;
use std::rc::Rc;
use std::sync::Arc;

use wsp_core::types::api::Runtime as _;
use wsp_core::types::wiring::RuntimeOptions;
use wsp_engine::fake_backend::{FakeBackend, FakeBackendOptions};
use wsp_engine::machine::MachineBackend;
use wsp_machines::places::types::{HerePlace, PlaceWiring};
use wsp_serve::create_runtime;
use wsp_store::keys::new_place_key_pair;
use wsp_store::store::Store;

fn options(dir: &Path) -> RuntimeOptions {
    let backend: Arc<dyn MachineBackend> = Arc::new(FakeBackend::new(FakeBackendOptions::default()));
    let store: Rc<dyn Store> = Rc::new(wsp_store::sqlite_store::sqlite_store(
        &dir.join("state.json"),
        wsp_store::store::StateWriter { wsp: "test".into(), daemon: 1.into(), bin: "/usr/local/bin/wsp".into() },
    ));
    let wiring = PlaceWiring {
        host_key: new_place_key_pair(),
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
    };
    RuntimeOptions { backend: Some(backend), store: Some(store), place_links: Some(Rc::new(wiring)), ..Default::default() }
}

#[test]
fn a_reset_on_a_word_that_names_no_computer_is_refused_in_the_place_words() {
    let dir = tempfile::tempdir().expect("a folder");
    let tokio_rt = tokio::runtime::Builder::new_current_thread().enable_all().build().expect("a tokio runtime");
    tokio::task::LocalSet::new().block_on(&tokio_rt, async {
        let rt = create_runtime(options(dir.path())).expect("a runtime");
        let said = rt.usage().reset("codex:nobody".into(), None, Some("no-such-computer".into())).await.expect_err("refused");
        assert!(
            said.message.starts_with("no place named no-such-computer; you have zingzys-mac"),
            "the TS throws noSuchPlaceRefusal before it reads the account; the Rust said: {}",
            said.message
        );
    });
}
