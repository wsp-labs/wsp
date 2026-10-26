// SPDX-License-Identifier: AGPL-3.0-only
//! Review of wsp-threads (#2448): a held start stopped while it waits for a slot. threads.ts:478-481 rejects the
//! wait with the stop's words (`capStop`, :489-495), and sessions.ts:679 `await waits` throws them out of the loop at
//! once. `SessionsCtx::cap_wait` answers a `Joined`, which cannot carry the rejection, so the port goes round the
//! loop again: the copy, worktree and kept checks ask the machine a second time before the stop mark is read.
#![allow(dead_code, unused_imports)]

#[path = "support_a/mod.rs"]
mod support;
include!("sessions_launch/world.rs");

// sessions.ts:713-717: `running.handle.steer` is read before anything is kept; a turn with no steer road keeps no
// message (keepSteer, threads.ts:1004-1007, writes it onto the turn, whose end sends it back), and the send queues.
// The port keeps the message first and learns the road is absent only from `handle.steer` answering None
// (start_launch.rs:768-772), so the same words ride the turn's end and the queued turn.
#[test]
fn a_running_turn_with_no_steer_road_keeps_no_message_and_the_send_queues() {
    run_local(async {
        let r = rig_with(adapter(|a| a.steers = true));
        let h0 = add_running(&r.host, "sess-0", "turn-0", "t1", json!({}), |_| {});
        steer_answers(&r, true);
        let join = r.spawn(steering_send(&r));
        settle().await;
        assert_eq!(r.host.hooks.called("keep_steer"), Vec::<String>::new(), "nothing is kept on a turn that cannot be steered");
        end_turn(&r.host, "sess-0", &h0);
        settle().await;
        let handle = ok(join.await.expect("the launch ran"));
        assert_eq!(handle.outcome(), SessionStartOutcome::Queued);
    });
}

#[test]
fn a_start_stopped_while_it_waits_for_a_slot_fails_at_once_with_the_stops_words() {
    run_local(async {
        let r = rig();
        let words = "stopped before it started, while mac was full";
        let stopped = Rc::new(Cell::new(false));
        let s = stopped.clone();
        r.host.hooks.set::<dyn Fn(&str) -> Option<CapHeld>>(
            "SessionsCtx::cap_held",
            Rc::new(move |_t: &str| if s.get() { Some(CapHeld { stopped: Some(words.to_string()), ..blank_hold() }) } else { None }),
        );
        r.host.hooks.set::<dyn Fn(&WorkspaceRecord, &str) -> Option<ThreadCapWait>>(
            "SessionsCtx::cap_full",
            Rc::new(|_r: &WorkspaceRecord, _t: &str| Some(full_wait())),
        );
        let (stop, waits) = gate();
        r.host.hooks.set::<dyn Fn(CapAt, ThreadCapWait) -> Joined>(
            "SessionsCtx::cap_wait",
            Rc::new(move |_a: CapAt, _f: ThreadCapWait| waits.clone()),
        );
        let mut p = prepared(&r);
        p.opts.on_held = Some(Rc::new(|_h: HeldStart| {}));
        let join = r.spawn(p);
        settle().await;
        assert_eq!(r.host.hooks.called("cap_wait").len(), 1, "the start waits for a slot");
        let checked = r.host.hooks.called("copy_blocked").len();
        stopped.set(true);
        stop.send(()).unwrap();
        settle().await;
        let e = err(join.await.expect("the launch ran"));
        assert_eq!(said(&e), (words.to_string(), None, None));
        assert_eq!(
            r.host.hooks.called("copy_blocked").len(),
            checked,
            "sessions.ts:679 throws the stop's words out of the loop: nothing is asked of the machine again"
        );
        assert_eq!(r.host.hooks.called("worktree_mounted").len(), checked);
    });
}
