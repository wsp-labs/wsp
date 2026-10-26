// SPDX-License-Identifier: AGPL-3.0-only
//! The group a turn on the shared daemon names is a process group id, which the channels area keys `i64` and the
//! runtime holds once (create_runtime packet 0b): turns.ts:651-657 sets `turnGroups.get(workspaceId).set(pid, threadId)`.
#![allow(dead_code, unused_imports)]

#[path = "support_a/mod.rs"]
mod support;
include!("turns_run/world.rs");

#[test]
fn a_shared_daemon_turn_names_its_group_by_the_pid_the_channels_area_keys() {
    run_local(async {
        let (host, entry) = world();
        host.hooks.set::<dyn Fn(&Entry) -> bool>("TurnsCtx::shared_daemon", Rc::new(|_e: &Entry| true));
        let turn = turn_row(json!({}));
        launching(&host, &turn);
        let _r = rig_on(&host, &entry, FakeSession::with(SESS, |s| s.pid = Some(4321)), turn, |_| {});
        settle().await;
        let held: Rc<RefCell<IndexMap<String, IndexMap<i64, String>>>> = host.turn_groups.clone();
        let mut group: IndexMap<i64, String> = IndexMap::new();
        group.insert(4321, THREAD.to_string());
        let mut groups = IndexMap::new();
        groups.insert(W.to_string(), group);
        assert_eq!(*held.borrow(), groups);
    });
}

#[allow(dead_code)]
fn the_turns_context_hands_the_groups_the_channels_area_keys(ctx: &dyn TurnsCtx) {
    let _: Rc<RefCell<IndexMap<String, IndexMap<i64, String>>>> = ctx.turn_groups();
}
