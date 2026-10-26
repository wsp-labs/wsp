// SPDX-License-Identifier: AGPL-3.0-only
//! Review of wsp-machines at 2c4f6cb46: the roads that touch a person's machines and money.

mod common;
#[path = "support/machines.rs"]
mod support;

use serde_json::json;
use support::{BackendOpts, Made, Opts, World};
use wsp_engine::errors::EngineError;
use wsp_machines::machines::area_error::AreaError;
use wsp_machines::machines::sub_spawn::machine::fault_of;
use wsp_words::sub_thread::SubFault;

/// sub-spawn.ts:285-308 with machines.ts:85-91: the provider's own error reaches `faultOf` whole through `ctx.fork`,
/// so a create the cloud cancelled names its box, `dropMachine` kills that box (made under a replayed key, so the
/// provider did not delete it, box-backend.ts:868-872), and the start is replaced once. `keyed_create` answers
/// `host_of(e)` (create.rs, the `at.create` error arm), so what `fork` answers can never be a fault.
#[test]
fn a_fork_the_cloud_cancelled_still_names_its_box_to_the_sub_thread_start() {
    common::run_local(async {
        let w = World::new(Opts {
            backend: Some(BackendOpts {
                creates: vec![Made::Fail(EngineError::CreateCancelled {
                    machine_id: "box-9".into(),
                    why: "out of capacity".into(),
                    line: "Boat cancelled w1: out of capacity".into(),
                })],
                ..Default::default()
            }),
            ..Default::default()
        });
        let record = support::shared(support::record_of("running", json!({}), true));
        let err = w
            .area
            .fork(&record, &|_m, _r| {}, None, None, false)
            .await
            .err()
            .expect("the cloud cancelled the create");
        let fault = fault_of(&AreaError::from(err));
        assert!(
            matches!(&fault, Some(SubFault::Cancelled { r#box, .. }) if r#box == "box-9"),
            "the fork's failure must still be the cancelled fault naming box-9, so the start kills it and tries once \
             more; got {fault:?}"
        );
    })
}

/// machines.ts:321 with create.ts:104-106 and sub-spawn.ts:423-426: `bind` writes `record.machineId` and attaches the
/// record it was handed, inside the fork's claim. The Rust fork calls `bind(&machine, &record.borrow().clone())`
/// (create.rs `fork_claimed`), and the `Ref` temporary lives until the statement ends, so a bind that writes the record
/// (the sub-thread start's `on_machine` does, sub_spawn/machine.rs, and `attach` does where the backend keeps no
/// image) panics with the record still borrowed. The create's bind dodges it by attaching a clone (review_create_bind).
#[test]
fn a_bind_that_writes_the_record_it_forks_does_not_panic() {
    common::run_local(async {
        let w = World::new(Opts {
            backend: Some(BackendOpts {
                creates: vec![Made::Machine(support::MachineDef::new("m-1"))],
                ..Default::default()
            }),
            ..Default::default()
        });
        let record = support::shared(support::record_of("running", json!({}), true));
        let held = record.clone();
        let bind = move |m: &std::sync::Arc<dyn wsp_engine::machine::Machine>,
                         _r: &wsp_core::types::wiring::WorkspaceRecord| {
            held.borrow_mut().machine_id = m.id().to_string();
        };
        let forked = futures::FutureExt::catch_unwind(std::panic::AssertUnwindSafe(
            w.area.fork(&record, &bind, None, None, false),
        ))
        .await;
        let machine = match forked {
            Ok(made) => made.expect("the fork lands"),
            Err(panic) => panic!(
                "the fork panicked inside bind: {}",
                panic
                    .downcast_ref::<String>()
                    .cloned()
                    .or_else(|| panic.downcast_ref::<&str>().map(|s| s.to_string()))
                    .unwrap_or_default()
            ),
        };
        assert_eq!(record.borrow().machine_id, machine.id().to_string());
    })
}
