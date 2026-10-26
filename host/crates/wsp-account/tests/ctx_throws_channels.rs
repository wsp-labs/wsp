// SPDX-License-Identifier: AGPL-3.0-only
//! The context's `adoptPause` throws through the channels area's failed dial (channels.ts:142). The throw replaces
//! the dial's own error, as a rejection in the TS catch does.
#![allow(dead_code, unused_imports)]
mod common;
include!("channels_daemon/world.rs");

// channels.ts:142: the adoption's rejection replaces the failed dial's error.
#[test]
fn ctx_throws_adopt_pause_replaces_the_dial_error() {
    run(async {
        let w = World::new();
        let area = w.area();
        let m = FakeMachine::new("m1");
        *m.state.borrow_mut() = Ok(MachineState::Paused);
        *w.road.borrow_mut() = Ok(reach());
        w.dials.borrow_mut().push_back(Err(plain("dial refused").into()));
        *w.adopt_fail.borrow_mut() = Some(plain("the provider refused the adoption"));
        let e = entry(&rec(json!({ "phase": "running" })), &m);
        let got = match area.own_daemon_channel(&e, quiet()).await {
            Ok(_) => panic!("the dial failed, so no channel opens"),
            Err(err) => HostError::from(err),
        };
        assert_eq!(got.message, "the provider refused the adoption");
        assert_eq!(w.adopted.borrow().len(), 1, "the pause was handed to adoption");
    });
}
