// SPDX-License-Identifier: AGPL-3.0-only
//! turn_road_of may fail (TS moduleOf(kind).turnRoad throws): the op token passes it through before any mint.
mod branch_support;
mod rest_support;

use std::cell::Cell;
use std::rc::Rc;

use branch_support::*;
use futures::FutureExt;
use rest_support::{plain_error, run};
use serde_json::json;
use wsp_words::exit::HostError;

const NOW: i64 = 1_700_000_000_000;

#[test]
fn a_failed_turn_road_passes_through_the_op_token_before_any_mint() {
    run(async {
        // branch-of-record.ts:156: the mint reads moduleOf(kind).turnRoad, which throws, with no catch.
        let rig = Rig::at(NOW);
        *rig.ctx.turn_road_failure.borrow_mut() =
            Some(plain_error("no module is wired for the kind cloud"));
        let (_, entry) = entry_of(
            &rig.ctx,
            json!({ "parentThreadId": "t_lead" }),
            Some("t_other"),
            false,
        );
        let ran = Rc::new(Cell::new(false));
        let flag = ran.clone();
        let got: Result<(), HostError> = rig
            .area
            .with_op_token(&entry, "clone", move |_| {
                flag.set(true);
                async { Ok(()) }.boxed_local()
            })
            .await;
        assert_eq!(
            got,
            Err(plain_error("no module is wired for the kind cloud"))
        );
        assert!(!ran.get(), "the run never starts when the road is refused");
        assert!(rig.ctx.mints().is_empty(), "no token is minted");
        assert!(rig.ctx.events().is_empty(), "nothing is revoked either");
    })
}
