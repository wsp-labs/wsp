// SPDX-License-Identifier: AGPL-3.0-only
//! Review of wsp-machines at 2c4f6cb46: two sends into a released sub-thread while its new machine fails.
#![allow(clippy::await_holding_refcell_ref)]
mod machines_b_support;

use machines_b_support::spawn_ctx::*;
use machines_b_support::*;

/// sub-spawn.ts:501-516: a send that finds a resume in flight awaits that resume's own promise, so where the start
/// fails, the second send is refused with the same failure. sub_spawn/machine.rs:544-556 waits on a gate that always
/// answers Ok (`let _ = ahead.await`) and answers the second send with the dead entry, so its turn is started on a
/// machine that does not exist.
#[test]
fn a_second_send_hears_the_failure_of_the_resume_it_waited_on() {
    run(async {
        let (f, entry) = released_sub();
        w().borrow_mut()
            .faults
            .extend([fault("unusable"), fault("unusable")]);
        let g = gate();
        *f.host.stand_up_gate.borrow_mut() = Some(g.clone());
        let (a_area, a_entry) = (f.area.clone(), entry.clone());
        let a = tokio::task::spawn_local(async move {
            a_area
                .resume_sub(&a_entry, "t_sub", "claude", Some("first"))
                .await
        });
        settle().await;
        let (b_area, b_entry) = (f.area.clone(), entry.clone());
        let b = tokio::task::spawn_local(async move {
            b_area
                .resume_sub(&b_entry, "t_sub", "claude", Some("second"))
                .await
        });
        settle().await;
        open(&g);
        let first = a.await.unwrap();
        let second = b.await.unwrap();
        let first = first.err().expect("the first start fails twice over");
        match second {
            Err(e) => assert_eq!(e.message, first.message),
            Ok(r) => panic!(
                "the second send went ahead on machine {:?} after the start it waited on failed: {}",
                r.entry.borrow().machine.id(),
                first.message
            ),
        }
    });
}
