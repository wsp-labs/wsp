// SPDX-License-Identifier: AGPL-3.0-only
//! The notes the channels area keeps of its daemons are the map the daemon area writes through the context
//! (channels.ts:225 `daemonNotes`, daemon.ts:352), and the roots recheck is armed on the runtime's own state
//! (context.ts:156 `rootsRecheck`, channels.ts:495), not on a handle of the area's own.
#![allow(dead_code, unused_imports)]
mod common;
include!("channels_ptys/world.rs");

#[test]
fn the_daemon_notes_an_area_keeps_are_the_map_it_hands_out() {
    run(async {
        let w = World::new();
        let area = w.area();
        let held: Rc<RefCell<IndexMap<String, String>>> = area.daemon_notes.clone();
        held.borrow_mut().insert("m1".into(), "no host cli".into());
        assert!(Rc::ptr_eq(&held, &area.daemon_notes));
        assert_eq!(area.daemon_notes.borrow().get("m1").map(String::as_str), Some("no host cli"));
    });
}
