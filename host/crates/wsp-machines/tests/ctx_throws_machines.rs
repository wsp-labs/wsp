// SPDX-License-Identifier: AGPL-3.0-only
//! backend_for and module_of may fail on settle and drop, which have no catch around the throw.
mod common;
#[path = "support/machines.rs"]
mod support;

use serde_json::json;
use support::{EntryOpts, Opts, World};
use wsp_engine::machine::MachineState;
use wsp_words::exit::HostError;

#[test]
fn an_unwired_backend_fails_the_settle_and_the_record_keeps_its_phase() {
    // machines.ts:702: backendFor in settleGone, with no try.
    common::run_local(async {
        let w = World::new(Opts {
            reads: Some(vec![Ok(MachineState::Gone)]),
            gone_confirm_ms: 0,
            ..Default::default()
        });
        let e = w.entry_in(EntryOpts {
            record: json!({ "firstLife": true }),
            ..Default::default()
        });
        let before = e.phase();
        *w.rt.backend_for_failure.borrow_mut() =
            Some(HostError::new("no backend is wired for the kind cloud"));
        let got = w
            .area
            .settle_gone(&e.entry, "machine m-1 is gone")
            .await
            .map(|_| ());
        assert_eq!(
            got,
            Err(HostError::new("no backend is wired for the kind cloud"))
        );
        assert_eq!(e.phase(), before, "the record is not marked gone");
    });
}

#[test]
fn an_unwired_kind_fails_the_drop_and_the_record_stays_held() {
    // machines.ts:532: moduleOf throws before the .catch on dropped, so drop rejects and nothing is deleted.
    common::run_local(async {
        let w = World::new(Opts::default());
        let _e = w.entry_in(EntryOpts::default());
        *w.rt.module_of_failure.borrow_mut() =
            Some(HostError::new("no module is wired for the kind cloud"));
        let got = w.area.drop("ws_1").await;
        assert_eq!(
            got,
            Err(HostError::new("no module is wired for the kind cloud"))
        );
        assert_eq!(
            w.live_ids(),
            vec!["ws_1".to_string()],
            "the record is still held"
        );
    });
}
