// SPDX-License-Identifier: AGPL-3.0-only
//! Part 2 of `threads_ported.rs`: the same world, the tests after it.
#![allow(dead_code, unused_imports)]

#[path = "support_b/mod.rs"]
mod support;
include!("threads_ported/world.rs");

// TS: thread-cap.test.ts:251, 297
#[test]
fn children_started_detached_run_one_at_a_time_in_order_and_a_followed_grandchild_takes_its_parents_slot() {
    let (ctx, area, record) = cap_host(1, 0);
    for (turn, thread) in [("n1", "c1"), ("n2", "c2"), ("n3", "c3")] {
        start_row(&ctx, turn, thread);
        area.cap_hold(&record, turn, None);
    }
    assert_eq!(area.cap_full(&record, "n1"), None, "the first in line has the computer");
    assert_eq!(area.cap_full(&record, "n2"), Some(cap_wait_of(0, 1)), "the turn ahead of it counts, though it runs nowhere yet");
    assert_eq!(area.cap_full(&record, "n3"), Some(cap_wait_of(0, 1)));
    area.cap_left("n1");
    assert_eq!(area.cap_full(&record, "n2"), Some(cap_wait_of(1, 1)), "now the first runs, and the next waits on it");
    // A child following its own child gets it started in its slot rather than waiting on itself.
    start_row(&ctx, "g1", "g1t");
    area.cap_hold(&record, "g1", Some("n1"));
    assert_eq!(area.cap_full(&record, "g1"), None);
    area.cap_left("g1");
    assert_eq!(borrowed_of(&area, "g1").as_deref(), Some("n1"));
    // One it started detached takes a slot of its own, behind the turns already in line.
    start_row(&ctx, "g2", "g2t");
    area.cap_hold(&record, "g2", None);
    assert_eq!(
        area.cap_full(&record, "g2"),
        Some(cap_wait_of(2, 1)),
        "two agents run here, one of them in the other's slot, and two turns stand ahead"
    );
    turn_ends(&ctx, "g1");
    turn_ends(&ctx, "n1");
    assert_eq!(area.cap_full(&record, "n2"), None, "the line moves in the order it came");
    area.cap_left("n2");
    assert_eq!(area.cap_full(&record, "n3"), Some(cap_wait_of(1, 1)));
}

// TS: thread-cap.test.ts:346
#[test]
fn a_chain_of_blocking_starts_four_deep_runs_each_in_the_slot_of_the_thread_waiting_on_it() {
    let (ctx, area, record) = cap_host(1, 1);
    let mut lender = "u0".to_string();
    for d in 1..=4 {
        let turn = format!("n{d}");
        start_row(&ctx, &turn, &format!("tl{d}"));
        area.cap_hold(&record, &turn, Some(lender.as_str()));
        assert_eq!(area.cap_full(&record, &turn), None, "depth {d}");
        area.cap_left(&turn);
        assert_eq!(borrowed_of(&area, &turn).as_deref(), Some(lender.as_str()), "depth {d}");
        lender = turn;
    }
    start_row(&ctx, "nx", "tx");
    area.cap_hold(&record, "nx", None);
    assert_eq!(area.cap_full(&record, "nx"), Some(cap_wait_of(5, 1)), "five agents run on a computer set to one, and a sixth thread waits");
}

// TS: thread-cap.test.ts:371
#[test]
fn a_turn_in_a_slot_it_borrowed_holds_one_of_its_own_when_the_thread_it_borrowed_from_ends_first() {
    let (ctx, area, record) = cap_host(1, 1);
    start_row(&ctx, "n1", "child");
    area.cap_hold(&record, "n1", Some("u0"));
    assert_eq!(area.cap_full(&record, "n1"), None);
    area.cap_left("n1");
    start_row(&ctx, "n2", "grand");
    area.cap_hold(&record, "n2", Some("n1"));
    assert_eq!(area.cap_full(&record, "n2"), None);
    area.cap_left("n2");
    start_row(&ctx, "nx", "outside");
    area.cap_hold(&record, "nx", None);
    assert_eq!(area.cap_full(&record, "nx"), Some(cap_wait_of(3, 1)));
    // The child's follow gives up and its turn ends with the grandchild still running.
    turn_ends(&ctx, "n1");
    assert_eq!(borrowed_of(&area, "n2"), None, "the grandchild lent from the child holds a slot of its own now");
    assert_eq!(area.cap_full(&record, "nx"), Some(cap_wait_of(2, 1)), "the lead and the grandchild, one slot each");
    turn_ends(&ctx, "u0");
    assert_eq!(area.cap_full(&record, "nx"), Some(cap_wait_of(1, 1)), "the grandchild alone, still holding the computer");
    turn_ends(&ctx, "n2");
    assert_eq!(area.cap_full(&record, "nx"), None);
}

// TS: thread-cap.test.ts:788, 808, 828, 848
#[test]
fn a_follow_up_sent_into_a_held_thread_lends_it_the_senders_slot_and_the_transcript_says_lent() {
    local(async {
        let (ctx, area, record) = cap_host(1, 1);
        start_row(&ctx, "n1", "tb");
        area.cap_hold(&record, "n1", None);
        let waiting = area.cap_wait(at("n1", "tb"), cap_wait_of(1, 1)).shared();
        let watch = waiting.clone();
        tokio::task::spawn_local(async move {
            let _ = watch.await;
        });
        settle().await;
        assert!(waiting.peek().is_none(), "held detached, the builder waits for a slot");
        area.cap_lend("n1", "u0");
        settle().await;
        assert!(matches!(waiting.peek(), Some(Ok(()))), "the lead follows the builder now, so the builder looks again");
        assert_eq!(area.cap_full(&record, "n1"), None, "it runs in the sender's slot");
        assert_eq!(borrowed_of(&area, "n1").as_deref(), Some("u0"));
        assert_eq!(area.cap_held.borrow().get("n1").and_then(|h| h.through), Some((SessionUncappedEventWhy::Lent, 1)));
        area.cap_left("n1");
        settle().await;
        let said = ctx.recorded_json();
        assert_eq!(said.iter().map(|e| e["type"].clone()).collect::<Vec<_>>(), vec![json!("session.capped"), json!("session.uncapped")]);
        for (k, v) in [
            ("workspaceId", json!("ws1")),
            ("threadId", json!("tb")),
            ("sessionId", json!("s-n1")),
            ("turnId", json!("n1")),
            ("why", json!("lent")),
            ("atOnce", json!(1)),
        ] {
            assert_eq!(said[1][k], v, "{k}");
        }
        assert_eq!(ctx.store.logged(), vec!["put held-starts/n1", "delete held-starts/n1"]);
    });
}

// TS: thread-cap.test.ts:445
#[test]
fn a_stop_that_lands_after_the_wake_is_read_by_the_next_look_and_the_turn_never_launches() {
    local(async {
        let (ctx, area, record) = cap_host(1, 1);
        start_row(&ctx, "n1", "tn");
        area.cap_hold(&record, "n1", None);
        let waiting = area.cap_wait(at("n1", "tn"), cap_wait_of(1, 1)).shared();
        let watch = waiting.clone();
        tokio::task::spawn_local(async move {
            let _ = watch.await;
        });
        settle().await;
        ctx.bus.emit_json(end_of("other"));
        settle().await;
        assert!(matches!(waiting.peek(), Some(Ok(()))), "an end anywhere woke the wait");
        // The stop lands between the wake and the look the wake is for.
        assert!(area.cap_stop("n1", "stopped before it started"));
        let next = area.cap_wait(at("n1", "tn"), cap_wait_of(1, 1)).await;
        assert_eq!(next.expect_err("the look reads the stop").message, "stopped before it started");
        settle().await;
        assert_eq!(ctx.recorded_json().len(), 1, "only the first wait was said");
        assert_eq!(ctx.store.logged(), vec!["put held-starts/n1"], "no second record written");
        assert_eq!(ctx.called().iter().filter(|c| *c == "idle.hold ws1").count(), 1, "the refused look holds the machine awake no more");
        area.cap_left("n1");
        settle().await;
        assert_eq!(ctx.store.logged(), vec!["put held-starts/n1", "delete held-starts/n1"]);
        assert_eq!(ctx.recorded_json().len(), 1, "a turn that never got through says no why");
    });
}

// TS: thread-cap.test.ts:685
#[test]
fn a_held_send_of_three_thousand_characters_keeps_them_all_in_its_record_and_its_wait() {
    local(async {
        let (ctx, area, record) = cap_host(1, 1);
        let long: String = format!("two {}", "wait for a slot and keep every word. ".repeat(80)).chars().take(3000).collect();
        assert_eq!(long.chars().count(), 2964, "node gives 2964 for the 3000-slice of this string");
        start_row(&ctx, "n1", "tn");
        area.cap_hold(&record, "n1", None);
        let _waiting = area.cap_wait(CapAt { prompt: long.clone(), ..at("n1", "tn") }, cap_wait_of(1, 1));
        settle().await;
        let held = ctx.store.get_now("held-starts", "n1").expect("written down");
        assert_eq!(held["prompt"], json!(long), "the record is the only copy of a message the agent never got");
        assert_eq!(held["turnId"], json!("n1"));
        let said = ctx.recorded_json();
        assert_eq!(
            (said[0]["type"].clone(), said[0]["turnId"].clone(), said[0]["prompt"].clone()),
            (json!("session.capped"), json!("n1"), json!(long))
        );
    });
}

// TS: thread-cap.test.ts:1003
#[test]
fn a_lenders_end_gives_the_turn_in_its_slot_a_slot_of_its_own_so_a_third_thread_waits() {
    let (ctx, area, record) = cap_host(2, 2);
    // u0 is the lead; u1 (thread t1) is another thread running beside it.
    start_row(&ctx, "nb", "tb");
    area.cap_hold(&record, "nb", None);
    start_row(&ctx, "nr", "tr");
    area.cap_hold(&record, "nr", Some("u0"));
    assert_eq!(area.cap_full(&record, "nr"), None, "the reviewer the lead follows runs in the lead's slot");
    area.cap_left("nr");
    // The reviewer asks the builder and follows: the builder runs in the reviewer's slot.
    area.cap_lend("nb", "nr");
    assert_eq!(area.cap_full(&record, "nb"), None);
    area.cap_left("nb");
    assert_eq!(borrowed_of(&area, "nb").as_deref(), Some("nr"));
    start_row(&ctx, "nx", "tx");
    area.cap_hold(&record, "nx", None);
    assert_eq!(area.cap_full(&record, "nx"), Some(cap_wait_of(4, 2)), "four agents, two of them lent: the third thread waits");
    // The reviewer is stopped while the builder runs in its slot.
    turn_ends(&ctx, "nr");
    assert_eq!(borrowed_of(&area, "nb"), None);
    assert_eq!(area.cap_full(&record, "nx"), Some(cap_wait_of(3, 2)), "lead, the other thread and the builder each hold a slot");
    turn_ends(&ctx, "u1");
    assert_eq!(area.cap_full(&record, "nx"), Some(cap_wait_of(2, 2)), "still full while the builder runs");
    turn_ends(&ctx, "nb");
    assert_eq!(area.cap_full(&record, "nx"), None);
}

// TS: thread-cap.test.ts:1098
#[test]
fn raising_the_threads_at_once_lets_the_waiting_turns_through_oldest_first_each_saying_raised() {
    local(async {
        let (ctx, area, record) = cap_host(6, 6);
        let mut waits = vec![];
        for (turn, thread, prompt) in [("n7", "t7", "seventh"), ("n8", "t8", "eighth")] {
            start_row(&ctx, turn, thread);
            area.cap_hold(&record, turn, None);
            let waiting = area.cap_wait(CapAt { prompt: prompt.into(), ..at(turn, thread) }, cap_wait_of(6, 6)).shared();
            let watch = waiting.clone();
            tokio::task::spawn_local(async move {
                let _ = watch.await;
            });
            waits.push(waiting);
        }
        settle().await;
        let said = ctx.recorded_json();
        assert_eq!(
            said.iter().map(|e| (e["turnId"].clone(), e["prompt"].clone())).collect::<Vec<_>>(),
            vec![(json!("n7"), json!("seventh")), (json!("n8"), json!("eighth"))]
        );
        // The person raises the count: every held turn looks again.
        ctx.threads_at.borrow_mut().insert("here".into(), 8);
        ctx.bus.emit_json(end_of("other"));
        settle().await;
        assert!(waits.iter().all(|w| matches!(w.peek(), Some(Ok(())))));
        assert_eq!(area.cap_full(&record, "n7"), None);
        assert_eq!(area.cap_full(&record, "n8"), None, "six running and one ahead make seven of eight");
        area.cap_left("n7");
        area.cap_left("n8");
        settle().await;
        let said = ctx.recorded_json();
        assert_eq!(
            said.iter().map(|e| e["type"].clone()).collect::<Vec<_>>(),
            vec![json!("session.capped"), json!("session.capped"), json!("session.uncapped"), json!("session.uncapped")]
        );
        for (i, turn) in [(2, "n7"), (3, "n8")] {
            assert_eq!(
                (said[i]["turnId"].clone(), said[i]["why"].clone(), said[i]["atOnce"].clone()),
                (json!(turn), json!("raised"), json!(8))
            );
        }
        assert_eq!(ctx.store.logged(), vec!["put held-starts/n7", "put held-starts/n8", "delete held-starts/n7", "delete held-starts/n8"]);
    });
}

// TS: thread-keeps-process.test.ts:92, 106
#[test]
fn a_kept_process_holds_its_turn_token_between_turns_and_the_next_turn_runs_under_the_same_one() {
    local(async {
        let (_ctx, area, _) = kept_host(&["t1"]);
        let (h, _agent) = kept_hand("ws1", None, "tok-one");
        assert!(area.hold_kept("t1", h).is_none());
        assert_eq!(area.thread_of_token("tok-one").expect("a slate write between turns lands on its own thread"), "t1");
        let process = area.take_kept("t1", Some(&same_launch()), Some("s1"), None).expect("the second send runs on it");
        assert_eq!(process.borrow().turn_token, "tok-one", "the process goes on holding the token its first turn was launched with");
        let refused = area.thread_of_token("tok-one").expect_err("while its turn runs the row carries the token, not the keep");
        assert_eq!(refused.message, NO_SUCH_TURN);
        let (again, _agent) = kept_hand("ws1", None, "tok-one");
        assert!(area.hold_kept("t1", again).is_none());
        assert_eq!(area.thread_of_token("tok-one").expect("put back after the turn"), "t1");
    });
}

// TS: thread-keeps-process.test.ts:161
#[test]
fn the_hosts_own_title_write_into_the_session_file_does_not_end_the_kept_process() {
    local(async {
        let dir = tempfile::tempdir().expect("a folder");
        let path = dir.path().join("s1.jsonl");
        std::fs::write(&path, b"{}\n").expect("written");
        let on_disk = SessionFile { folder: dir.path().to_string_lossy().into_owned(), name: "s1.jsonl".into(), depth: 0 };
        let (_ctx, area, _) = kept_host(&["t1"]);
        let (h, agent) = kept_hand("ws1", Some(on_disk), "tok");
        assert!(area.hold_kept("t1", h).is_none());
        for (what, text) in [
            ("the title", "{}\n{\"type\":\"custom-title\"}\n"),
            ("a rename the person makes", "{}\n{\"type\":\"custom-title\"}\n{\"type\":\"custom-title\"}\n"),
        ] {
            let at = path.clone();
            let wrote = area
                .write_session(
                    "s1",
                    async move {
                        std::fs::write(&at, text.as_bytes()).expect("the host wrote the file");
                        Ok(SessionRenameWrite::Written)
                    }
                    .boxed_local(),
                )
                .await;
            assert_eq!(wrote.expect("written"), SessionRenameWrite::Written, "{what}");
            settle().await;
            assert_eq!(kept_ids(&area), vec!["t1"], "{what}");
        }
        let taken = area.take_kept("t1", Some(&same_launch()), Some("s1"), None);
        assert!(taken.is_some(), "the next send runs on the kept process, not read as the person resuming the session elsewhere");
        assert!(agent.closes().is_empty());
    });
}

// TS: thread-keeps-process.test.ts:196
#[test]
fn an_idle_kept_process_ends_at_the_end_of_its_keep_and_its_token_then_names_nobody() {
    local(async {
        let (ctx, area, _) = kept_host(&["t1"]);
        let (h, agent) = kept_hand("ws1", None, "tok-idle");
        assert!(area.hold_kept("t1", h).is_none());
        ctx.clock.advance(AGENT_KEEP_MS - 1_000);
        settle().await;
        assert_eq!(area.thread_of_token("tok-idle").expect("between turns the token still names its thread"), "t1");
        assert!(agent.closes().is_empty());
        ctx.clock.advance(1_000);
        settle().await;
        assert!(kept_ids(&area).is_empty());
        assert_eq!(agent.closes(), vec![false]);
        assert_eq!(area.thread_of_token("tok-idle").expect_err("the keep ran out").message, NO_SUCH_TURN);
    });
}

// TS: thread-keeps-process.test.ts:270
#[test]
fn at_the_cap_the_idle_one_kept_longest_ends_and_never_a_process_still_working() {
    local(async {
        let names = ["t0", "t1", "t2", "t3", "t4", "t5", "t6"];
        let (ctx, area, _) = kept_host(&names);
        let mut agents = vec![];
        for (i, t) in names.iter().enumerate() {
            let (h, a) = kept_hand("ws1", None, &format!("tok-{i}"));
            if i == 0 {
                a.working.store(true, Ordering::SeqCst);
            }
            assert!(area.hold_kept(t, h).is_none());
            agents.push(a);
            ctx.clock.advance(1);
        }
        settle().await;
        assert_eq!(AGENTS_KEPT, 6);
        assert_eq!(
            kept_ids(&area),
            vec!["t0", "t2", "t3", "t4", "t5", "t6"],
            "the oldest is working, so the idle one kept longest after it goes"
        );
        assert_eq!(agents[1].closes(), vec![false]);
        assert!(agents[0].closes().is_empty());
    });
}

// TS: thread-warm-agent.test.ts:82
#[test]
fn a_claimed_warm_process_runs_the_send_that_opens_its_thread_and_is_kept_for_the_threads_next() {
    local(async {
        let (ctx, area, _) = kept_host(&[]);
        let (h, agent) = warm_kept_hand("ws1");
        area.hold_warm("w1", h);
        assert_eq!(area.claim_warm("ws1", "claude"), Some("w1".to_string()), "the send that opens a thread takes its thread id");
        let process = area
            .take_kept("w1", None, None, Some(&launch("claude|/work|v1", json!({ "model": "opus" }))))
            .expect("launched exactly as the send would launch");
        assert_eq!(process.borrow().warm.as_ref().map(|w| w.claimed), Some(true));
        assert!(kept_ids(&area).is_empty(), "taken out of the keep for its turn");
        assert_eq!(ctx.clock.pending(), 0, "its idle window is cancelled");
        assert!(agent.closes().is_empty());
        assert_eq!(area.warm_on("ws1", "claude").map(|(t, _)| t), None);
        // The turn over, the thread's own process is kept for its next send.
        ctx.thread_records.with(|r| {
            r.insert("w1".into(), record("ws1", json!({})));
        });
        let (next, _a) = kept_hand("ws1", None, "tok-warm");
        assert!(area.hold_kept("w1", next).is_none());
        assert_eq!(kept_ids(&area), vec!["w1"]);
        assert_eq!(area.warm_on("ws1", "claude").map(|(t, _)| t), None, "kept as a thread's own, not as a warm one");
    });
}

// TS: thread-warm-agent.test.ts:100
#[test]
fn a_warm_process_never_outlives_its_three_idle_minutes() {
    local(async {
        let (ctx, area, _) = kept_host(&[]);
        let (h, agent) = warm_kept_hand("ws1");
        area.hold_warm("w1", h);
        ctx.clock.advance(AGENT_WARM_MS - 1_000);
        settle().await;
        assert_eq!(kept_ids(&area), vec!["w1"]);
        assert!(agent.closes().is_empty());
        ctx.clock.advance(1_000);
        settle().await;
        assert!(kept_ids(&area).is_empty());
        assert_eq!(agent.closes(), vec![false]);
        assert_eq!(area.claim_warm("ws1", "claude"), None, "a send after it boots a process of its own");
        let (again, _a) = warm_kept_hand("ws1");
        area.hold_warm("w1", again);
        assert_eq!(area.claim_warm("ws1", "claude"), Some("w1".to_string()), "asked for again, a new one stands");
    });
}

// TS: thread-warm-agent.test.ts:124
#[test]
fn asked_for_a_minute_apart_for_five_minutes_the_send_runs_on_the_process_started_at_the_first() {
    local(async {
        let (ctx, area, _) = kept_host(&[]);
        let (h, agent) = warm_kept_hand("ws1");
        area.hold_warm("w1", h);
        for _ in 1..=5 {
            ctx.clock.advance(60_000);
            area.rewarm("w1");
            assert_eq!(ctx.clock.pending(), 1, "each ask replaces the window, never adds one");
        }
        ctx.clock.advance(AGENT_WARM_MS - 1_000);
        settle().await;
        assert_eq!(kept_ids(&area), vec!["w1"]);
        assert!(agent.closes().is_empty());
        assert_eq!(area.claim_warm("ws1", "claude"), Some("w1".to_string()));
        assert!(area.take_kept("w1", None, None, Some(&launch("claude|/work|v1", json!({ "model": "opus" })))).is_some());
    });
}

// TS: thread-warm-agent.test.ts:162, 174
#[test]
fn a_send_at_other_picks_than_the_warm_process_was_launched_with_ends_it_either_way_round() {
    local(async {
        for (kept_picks, sent_picks) in [
            (json!({ "model": "opus" }), json!({ "model": "sonnet" })),
            (json!({ "model": "opus", "fast": true }), json!({ "model": "opus" })),
        ] {
            let (_ctx, area, _) = kept_host(&[]);
            let (mut h, agent) = warm_kept_hand("ws1");
            h.launch = launch("claude|/work|v1", kept_picks.clone());
            area.hold_warm("w1", h);
            assert!(
                area.take_kept("w1", None, None, Some(&launch("claude|/work|v1", sent_picks.clone()))).is_none(),
                "{kept_picks} against {sent_picks}"
            );
            settle().await;
            assert!(kept_ids(&area).is_empty());
            assert_eq!(agent.closes(), vec![false], "it ends rather than run on the wrong model, or with a pick nobody asked for");
        }
    });
}

// TS: thread-warm-agent.test.ts:186
#[test]
fn a_send_that_claimed_the_warm_process_and_is_refused_ends_it_so_none_stands_claimed() {
    local(async {
        let (_ctx, area, _) = kept_host(&[]);
        let (h, agent) = warm_kept_hand("ws1");
        area.hold_warm("w1", h);
        assert_eq!(area.claim_warm("ws1", "claude"), Some("w1".to_string()));
        area.reap_kept("w1", false);
        settle().await;
        assert!(kept_ids(&area).is_empty());
        assert_eq!(agent.closes(), vec![false]);
        assert_eq!(area.warm_on("ws1", "claude").map(|(t, _)| t), None);
        let (fresh, _a) = warm_kept_hand("ws1");
        area.hold_warm("w2", fresh);
        assert_eq!(area.claim_warm("ws1", "claude"), Some("w2".to_string()), "the next warm one is claimable");
    });
}

// TS: thread-warm-agent.test.ts:252
#[test]
fn warming_a_third_ends_the_one_asked_for_longest_ago_and_asking_again_moves_one_up() {
    local(async {
        let (ctx, area, _) = kept_host(&[]);
        let mut agents = vec![];
        for t in ["w1", "w2", "w3"] {
            let (h, a) = warm_kept_hand("ws1");
            area.hold_warm(t, h);
            agents.push(a);
            ctx.clock.advance(10);
        }
        settle().await;
        assert_eq!(kept_ids(&area), vec!["w2", "w3"]);
        assert_eq!(agents[0].closes(), vec![false]);
        // Asked for again, w2 is the more recent of the two, so the fourth ends w3.
        area.rewarm("w2");
        ctx.clock.advance(10);
        let (h, a) = warm_kept_hand("ws1");
        area.hold_warm("w4", h);
        agents.push(a);
        settle().await;
        assert_eq!(kept_ids(&area), vec!["w2", "w4"]);
        assert_eq!(agents[2].closes(), vec![false]);
        assert!(agents[1].closes().is_empty() && agents[3].closes().is_empty());
    });
}

// TS: thread-warm-agent.test.ts:276
#[test]
fn five_warm_processes_asked_in_turn_leave_two_standing_and_those_count_against_the_cap() {
    local(async {
        let threads = ["k1", "k2", "k3", "k4", "k5"];
        let (ctx, area, _) = kept_host(&threads);
        let mut warm = vec![];
        for t in ["w1", "w2", "w3", "w4", "w5"] {
            let (h, a) = warm_kept_hand("ws1");
            area.hold_warm(t, h);
            warm.push(a);
            ctx.clock.advance(10);
        }
        settle().await;
        assert_eq!(kept_ids(&area), vec!["w4", "w5"]);
        for a in &warm[..3] {
            assert_eq!(a.closes(), vec![false]);
        }
        // Five threads' kept processes on top of the two: the seventh process ends the idle one kept longest, the older warm one.
        for t in threads {
            let (h, _a) = kept_hand("ws1", None, &format!("tok-{t}"));
            assert!(area.hold_kept(t, h).is_none());
            ctx.clock.advance(10);
        }
        settle().await;
        assert_eq!(warm[3].closes(), vec![false]);
        assert!(warm[4].closes().is_empty());
        assert_eq!(kept_ids(&area), vec!["w5", "k1", "k2", "k3", "k4", "k5"]);
    });
}

// TS: box-keep.test.ts:53
#[test]
fn a_joined_computers_second_send_runs_on_the_process_its_first_turn_left_up_with_no_new_launch() {
    local(async {
        let (_ctx, area, machine) = kept_host(&["t1"]);
        machine.answer("wc -c", 0, "100");
        let (h, agent) = kept_hand("wsb", Some(session_file()), "tok");
        assert!(area.hold_kept("t1", h).is_none());
        settle().await;
        area.kept_check("t1").await;
        assert_eq!(kept_ids(&area), vec!["t1"], "the file there stands as the turn left it");
        let taken = area.take_kept("t1", Some(&same_launch()), Some("s1"), None).expect("the second send runs on it");
        assert_eq!(taken.borrow().turn_token, "tok");
        assert_eq!(wc_reads(&machine), 2, "one read at the keep and one at the send's check: taking asks the computer nothing");
        assert!(agent.closes().is_empty());
    });
}

// TS: box-keep.test.ts:62
#[test]
fn a_joined_computer_keeps_at_most_what_its_memory_takes_and_counts_only_its_own() {
    local(async {
        let (ctx, area, _) = kept_host(&["t0", "t1", "t2", "t3"]);
        let mut agents = vec![];
        for t in ["t0", "t1", "t2"] {
            let (h, a) = kept_hand("wsb", None, &format!("tok-{t}"));
            assert!(area.hold_kept(t, h).is_none());
            agents.push(a);
            ctx.clock.advance(1);
        }
        settle().await;
        assert_eq!(kept_ids(&area), vec!["t1", "t2"], "4096 MB keeps 2 there, the one idle longest ended first");
        assert_eq!(agents[0].closes(), vec![false]);
        let (h, _a) = kept_hand("ws1", None, "tok-here");
        assert!(area.hold_kept("t3", h).is_none());
        settle().await;
        assert_eq!(kept_ids(&area), vec!["t1", "t2", "t3"], "a process on this computer is no part of the box's count");
    });
}

// TS: box-keep.test.ts:109
#[test]
fn a_keep_that_runs_out_while_the_computer_is_away_closes_the_agent_at_its_next_link_and_owes_nothing() {
    local(async {
        let (ctx, area, machine) = kept_host(&["t1"]);
        let (h, agent) = kept_hand("wsb", None, "tok");
        assert!(area.hold_kept("t1", h).is_none());
        ctx.linked.borrow_mut().clear();
        ctx.clock.advance(AGENT_KEEP_MS);
        settle().await;
        assert!(kept_ids(&area).is_empty());
        assert!(agent.closes().is_empty(), "nothing reaches a computer that is away");
        assert_eq!(ctx.place_listeners.borrow().len(), 1);
        assert!(ctx.store.logged().is_empty(), "no thread end is owed the computer");
        assert!(!machine.commands().iter().any(|c| c.contains("wsp_end")));
        ctx.place_move(wsp_threads::threads::threads::PlaceMove::Present("box1".into()));
        settle().await;
        assert_eq!(agent.closes(), vec![false], "closed at the next link, ending the run's own group as it would have then");
        assert!(ctx.place_listeners.borrow().is_empty());
        assert!(ctx.store.logged().is_empty());
    });
}

// TS: box-keep.test.ts:119
#[test]
fn a_send_while_the_computer_is_away_boots_cold_owes_no_thread_end_and_the_old_agent_closes_at_its_next_link() {
    local(async {
        let (ctx, area, machine) = kept_host(&["t1"]);
        let (h, agent) = kept_hand("wsb", None, "tok");
        assert!(area.hold_kept("t1", h).is_none());
        ctx.linked.borrow_mut().clear();
        area.kept_check("t1").await;
        assert!(kept_ids(&area).is_empty(), "its message would reach nothing, so the send boots cold");
        assert!(area.take_kept("t1", Some(&same_launch()), Some("s1"), None).is_none());
        assert!(agent.closes().is_empty());
        assert!(ctx.store.logged().is_empty());
        assert!(machine.commands().is_empty());
        ctx.place_move(wsp_threads::threads::threads::PlaceMove::Present("box1".into()));
        settle().await;
        assert_eq!(agent.closes(), vec![false]);
    });
}

// TS: box-keep.test.ts:128
#[test]
fn a_send_waits_out_the_hosts_own_write_into_the_session_file_before_reading_it() {
    local(async {
        let (_ctx, area, machine) = kept_host(&["t1"]);
        machine.answer("wc -c", 0, "100");
        let (h, agent) = kept_hand("wsb", Some(session_file()), "tok");
        assert!(area.hold_kept("t1", h).is_none());
        settle().await;
        assert_eq!(wc_reads(&machine), 1);
        let (tx, rx) = futures::channel::oneshot::channel::<()>();
        let write = area.write_session(
            "s1",
            async move {
                let _ = rx.await;
                Ok(SessionRenameWrite::Written)
            }
            .boxed_local(),
        );
        tokio::task::spawn_local(write);
        settle().await;
        // The title is in the file at once and its answer comes later, so a send meanwhile would read a grown file.
        machine.rules.borrow_mut().clear();
        machine.answer("wc -c", 0, "150");
        let check = tokio::task::spawn_local({
            let area = area.clone();
            async move { area.kept_check("t1").await }
        });
        settle().await;
        assert!(!check.is_finished(), "the send waits for the write");
        assert_eq!(wc_reads(&machine), 1, "it reads nothing before the write answered");
        let _ = tx.send(());
        settle().await;
        assert!(check.is_finished());
        assert_eq!(
            kept_ids(&area),
            vec!["t1"],
            "the send runs on the kept agent: the host's own write is not the person resuming the session"
        );
        assert_eq!(wc_reads(&machine), 3, "one at the keep, one restamping after the write, one at the check");
        assert!(agent.closes().is_empty());
    });
}

// TS: turn-changes.test.ts:186, 241
#[test]
fn a_snapshot_the_daemon_never_answers_ends_at_the_options_cap_and_the_turn_records_nothing() {
    local(async {
        let (ctx, area, e) = te_host();
        ctx.snapshot_ms.set(Some(30));
        ctx.daemon_hangs.borrow_mut().push("git.snapshot".into());
        let warned = wsp_core::console::capture();
        let snapshot = tokio::task::spawn_local({
            let (area, e) = (area.clone(), e.clone());
            async move { area.snapshot_of(&e, "/w").await }
        });
        settle().await;
        ctx.clock.advance(29);
        settle().await;
        assert!(!snapshot.is_finished(), "the cap is the option's, not the default 3000 ms");
        ctx.clock.advance(1);
        settle().await;
        assert_eq!(snapshot.await.expect("ran"), None);
        assert_eq!(
            warned.lines(),
            vec!["no snapshot of /w on the-folder inside 30ms; the turn runs without one and its reply lists no changes"]
        );
        let read = tokio::task::spawn_local({
            let (area, e) = (area.clone(), e.clone());
            async move { area.read_turn_changes(&e, turn_ask("mine", "/w", None)).await }
        });
        settle().await;
        ctx.clock.advance(30);
        settle().await;
        assert!(!read.await.expect("ran"), "no snapshot at the end either");
        assert!(!ctx.frames.borrow().iter().any(|f| f["op"] == "git.turn"), "the range is never asked");
        assert!(ctx.recorded.borrow().is_empty());
    });
}

// TS: turn-changes.test.ts:230
#[test]
fn a_turn_that_only_moved_head_records_no_files_of_its_own_and_the_move_named_on_the_card() {
    local(async {
        let (ctx, area, e) = te_host();
        ctx.daemon_answers("git.snapshot", Ok(json!({ "commit": "snap1" })));
        ctx.daemon_answers("git.turn", Ok(diff(json!([]), json!(["Pulled"]))));
        assert!(area.read_turn_changes(&e, turn_ask("mine", "/repo/a", None)).await);
        let said = ctx.recorded_json();
        assert_eq!(said.len(), 1);
        assert_eq!(
            (said[0]["type"].clone(), said[0]["files"].clone(), said[0]["moved"].clone()),
            (json!("session.changes"), json!([]), json!(["Pulled"]))
        );
        assert_eq!(said[0].get("shared"), None);
        assert_eq!(said[0].get("others"), None);
    });
}

// TS: turn-changes.test.ts:254, 333
#[test]
fn a_turn_shares_its_folder_with_another_threads_turn_and_keeps_the_whole_list_where_the_agent_reports_no_edits() {
    local(async {
        let (ctx, area, e) = te_host();
        beside(&ctx, vec![json!({ "threadId": "other", "cwd": "/repo/a", "startedAt": 999_500 })]);
        ctx.daemon_answers("git.snapshot", Ok(json!({ "commit": "snap1" })));
        ctx.daemon_answers("git.turn", Ok(diff(json!([changed("a.ts")]), json!([]))));
        assert!(area.read_turn_changes(&e, turn_ask("mine", "/repo/a", None)).await);
        let said = ctx.recorded_json();
        assert_eq!(said.len(), 1);
        assert_eq!(said[0]["files"], json!([bare("a.ts")]), "the folder's list, not split");
        assert_eq!(said[0]["shared"], json!(true));
        assert_eq!(said[0].get("others"), None);
    });
}

// TS: turn-changes.test.ts:293
#[test]
fn a_turn_that_wrote_nothing_shows_none_of_its_own_while_the_thread_that_changed_the_folder_keeps_its_own() {
    local(async {
        let (ctx, area, e) = te_host();
        beside(
            &ctx,
            vec![
                json!({ "threadId": "quiet", "cwd": "/repo/a", "startedAt": 999_100 }),
                json!({ "threadId": "busy", "cwd": "/repo/a", "startedAt": 999_200 }),
            ],
        );
        let (worktrees, _) = top_of_repo();
        ctx.daemon_answers("git.snapshot", Ok(json!({ "commit": "snap1" })));
        ctx.daemon_answers("git.turn", Ok(diff(json!([changed("a/b.ts")]), json!([]))));
        ctx.daemon_answers("git.worktrees", Ok(worktrees));
        assert!(area.read_turn_changes(&e, turn_ask("quiet", "/repo/a", Some(&[]))).await);
        assert!(area.read_turn_changes(&e, turn_ask("busy", "/repo/a", Some(&["/repo/a/b.ts"]))).await);
        let said = ctx.recorded_json();
        assert_eq!(said.len(), 2);
        assert_eq!(
            (said[0]["files"].clone(), said[0]["others"].clone(), said[0]["shared"].clone()),
            (json!([]), json!([bare("a/b.ts")]), json!(true))
        );
        assert_eq!(
            (said[1]["files"].clone(), said[1]["others"].clone(), said[1]["shared"].clone()),
            (json!([bare("a/b.ts")]), json!([]), json!(true)),
            "an empty others is still said where the list was split"
        );
    });
}

// TS: turn-changes.test.ts:311
#[test]
fn two_threads_in_one_repo_at_different_subfolders_split_the_whole_checkouts_range_by_what_each_wrote() {
    local(async {
        let (ctx, area, e) = te_host();
        beside(
            &ctx,
            vec![
                json!({ "threadId": "ina", "cwd": "/repo/a", "startedAt": 999_100 }),
                json!({ "threadId": "inb", "cwd": "/repo/b", "startedAt": 999_200 }),
            ],
        );
        let (worktrees, _) = top_of_repo();
        ctx.daemon_answers("git.snapshot", Ok(json!({ "commit": "snap1" })));
        ctx.daemon_answers("git.turn", Ok(diff(json!([changed("a/x.ts"), changed("b/y.ts")]), json!([]))));
        ctx.daemon_answers("git.worktrees", Ok(worktrees));
        assert!(area.read_turn_changes(&e, turn_ask("ina", "/repo/a", Some(&["/repo/a/x.ts"]))).await);
        assert!(area.read_turn_changes(&e, turn_ask("inb", "/repo/b", Some(&["/repo/b/y.ts"]))).await);
        let said = ctx.recorded_json();
        assert_eq!(
            (said[0]["files"].clone(), said[0]["others"].clone(), said[0]["shared"].clone()),
            (json!([bare("a/x.ts")]), json!([bare("b/y.ts")]), json!(true))
        );
        assert_eq!(
            (said[1]["files"].clone(), said[1]["others"].clone(), said[1]["shared"].clone()),
            (json!([bare("b/y.ts")]), json!([bare("a/x.ts")]), json!(true))
        );
    });
}

// TS: turn-changes.test.ts:349
#[test]
fn every_change_is_the_turns_own_where_no_other_thread_ran_in_its_folder_edits_it_never_reported_included() {
    local(async {
        let (ctx, area, e) = te_host();
        ctx.daemon_answers("git.snapshot", Ok(json!({ "commit": "snap1" })));
        ctx.daemon_answers("git.turn", Ok(diff(json!([changed("a.ts"), changed("dist/out.js")]), json!([]))));
        assert!(area.read_turn_changes(&e, turn_ask("mine", "/repo/a", Some(&["/repo/a/a.ts"]))).await);
        let said = ctx.recorded_json();
        assert_eq!(said.len(), 1);
        assert_eq!(
            said[0]["files"],
            json!([bare("a.ts"), bare("dist/out.js")]),
            "the build output the agent never reported stays the turn's own"
        );
        assert_eq!(said[0].get("others"), None);
        assert_eq!(said[0].get("shared"), None);
    });
}

// TS: turn-changes.test.ts:512
#[test]
fn a_folder_git_holds_no_repo_in_takes_no_snapshot_and_reads_no_range() {
    local(async {
        let (ctx, area, e) = te_host();
        ctx.projects.borrow_mut().insert("p1".into(), held_project("p1", None));
        ctx.daemon_answers("git.snapshot", Ok(json!({ "commit": "snap1" })));
        assert_eq!(area.snapshot_of(&e, "/plain").await, None);
        assert!(!area.read_turn_changes(&e, turn_ask("mine", "/plain", None)).await);
        assert!(ctx.frames.borrow().is_empty(), "no snapshot, no range and no checkpoint is asked of the daemon");
        assert!(ctx.recorded.borrow().is_empty());
        // A folder git does hold is asked.
        ctx.projects.borrow_mut().insert("p1".into(), held_project("p1", Some("/repo")));
        assert_eq!(area.snapshot_of(&e, "/repo").await.as_deref(), Some("snap1"));
        assert_eq!(ctx.frames.borrow().len(), 1);
    });
}

// TS: spawn-guard.test.ts:498
#[test]
fn a_thread_may_send_into_a_thread_of_its_own_tree_however_deep_it_sits() {
    let ctx = FakeThreadsCtx::new();
    roots(&ctx, &[("root", "root"), ("child", "root"), ("grand", "root")]);
    let area = ctx.area();
    let root = scoped("root", "ws1", "root");
    assert!(area.drives_thread(Some("root"), Some(&root)), "its own thread");
    assert!(area.drives_thread(Some("grand"), Some(&root)), "however deep");
    let deep = scoped("grand", "ws1", "root");
    assert!(area.drives_thread(Some("child"), Some(&deep)));
    assert!(area.drives_thread(Some("root"), Some(&deep)), "and up to the lead");
}

// TS: spawn-guard.test.ts:1246
#[test]
fn a_threads_own_token_drives_its_thread_and_the_tree_under_it_and_reads_nothing_else_on_its_workspace() {
    let ctx = FakeThreadsCtx::new();
    roots(&ctx, &[("mine", "mine"), ("other", "other"), ("root", "root"), ("child", "root"), ("grand", "root")]);
    ctx.sessions.with(|s| {
        *s = rows_of(vec![
            row(view(json!({ "id": "r-child", "threadId": "child", "parentThreadId": "root" })), "u1"),
            row(view(json!({ "id": "r-grand", "threadId": "grand", "parentThreadId": "child" })), "u2"),
        ])
    });
    let area = ctx.area();
    let lead = scoped("root", "ws1", "root");
    for thread in ["root", "child", "grand"] {
        assert!(area.drives_thread(Some(thread), Some(&lead)), "{thread}");
    }
    for thread in ["mine", "other"] {
        assert!(!area.drives_thread(Some(thread), Some(&lead)), "{thread} is another tree's, which reads as no thread at all");
    }
    for thread in ["mine", "other", "root", "child", "grand"] {
        assert!(area.drives_thread(Some(thread), None), "the person drives every thread");
    }
    assert_eq!(area.tree_under("root"), vec!["child", "grand"]);
}

// TS: spawn-guard.test.ts:1309
#[test]
fn a_notify_target_outside_the_callers_tree_reads_as_no_thread_even_when_the_callers_own_rows_are_gone() {
    let ctx = FakeThreadsCtx::new();
    roots(&ctx, &[("root", "root"), ("mine", "mine"), ("builder", "root")]);
    let area = ctx.area();
    let ghost = scoped("t_gone", "ws1", "root");
    assert!(
        !area.drives_thread(Some("mine"), Some(&ghost)),
        "a caller whose rows this host no longer holds still reads its root off the scope"
    );
    assert!(area.drives_thread(Some("root"), Some(&ghost)));
    assert!(area.drives_thread(Some("builder"), Some(&ghost)));
    assert!(area.drives_thread(Some("mine"), None), "a person names any thread, as today");
}

// TS: spawn-guard.test.ts:1351
#[test]
fn a_notify_target_may_be_the_parent_a_sibling_or_a_child_but_not_the_persons_own_thread() {
    let ctx = FakeThreadsCtx::new();
    roots(&ctx, &[("root", "root"), ("child", "root"), ("sibling", "root"), ("grand", "root"), ("mine", "mine")]);
    let area = ctx.area();
    let as_child = scoped("child", "ws1", "root");
    for target in ["root", "sibling", "grand"] {
        assert!(area.drives_thread(Some(target), Some(&as_child)), "{target}");
    }
    assert!(!area.drives_thread(Some("mine"), Some(&as_child)));
}

// TS: spawn-guard.test.ts:1438
#[test]
fn a_child_on_the_copy_its_lead_forked_reaches_the_lead_on_another_workspace_and_the_lead_reaches_it_back() {
    let ctx = FakeThreadsCtx::new();
    roots(&ctx, &[("lead", "lead"), ("child", "lead")]);
    ctx.sessions.with(|s| {
        *s =
            rows_of(vec![row(view(json!({ "id": "r-child", "threadId": "child", "parentThreadId": "lead", "workspaceId": "copy" })), "u1")])
    });
    let area = ctx.area();
    let as_child = scoped("child", "copy", "lead");
    let as_lead = scoped("lead", "ws1", "lead");
    assert!(
        area.drives_thread(Some("lead"), Some(&as_child)),
        "the scope's workspace is the copy, and the lead's thread is reached all the same"
    );
    assert!(area.drives_thread(Some("child"), Some(&as_lead)));
    assert!(area.settles_thread("child", Some(&as_lead)));
    assert!(!area.settles_thread("lead", Some(&as_child)), "settling is never the lead's");
}

// TS: spawn-guard.test.ts:1471
#[test]
fn the_childs_reach_is_the_thread_alone_the_persons_thread_another_leads_tree_and_a_threadless_row_read_as_absent() {
    let ctx = FakeThreadsCtx::new();
    roots(&ctx, &[("lead", "lead"), ("child", "lead"), ("mine", "mine"), ("other", "other")]);
    let area = ctx.area();
    let as_child = scoped("child", "copy", "lead");
    assert!(!area.drives_thread(Some("mine"), Some(&as_child)));
    assert!(!area.drives_thread(Some("other"), Some(&as_child)));
    assert!(!area.drives_thread(None, Some(&as_child)), "a row with no thread of its own is in nobody's tree");
    assert!(!area.drives_thread(Some("unknown"), Some(&as_child)), "a thread no root is held for");
}

// TS: spawn-guard.test.ts:1535
#[test]
fn a_child_that_stops_its_lead_stops_the_whole_tree_its_sibling_and_itself_included() {
    local(async {
        let ctx = FakeThreadsCtx::new();
        ctx.sessions.with(|s| {
            *s = rows_of(vec![
                row(view(json!({ "id": "r-child", "threadId": "child", "parentThreadId": "root", "status": "running" })), "u1"),
                row(view(json!({ "id": "r-sibling", "threadId": "sibling", "parentThreadId": "root", "status": "running" })), "u2"),
            ])
        });
        let area = ctx.area();
        let stopped = area.stop_under("root", Some(scoped("child", "ws1", "root"))).await;
        let mut named = stopped.clone();
        named.sort();
        assert_eq!(named, vec!["child", "sibling"], "the answer names them");
        assert_eq!(ctx.called(), vec!["interrupt r-sibling", "interrupt r-child"], "deepest first, each through its own row");
    });
}

// TS: spawn-guard.test.ts:1553
#[test]
fn stopping_a_root_ends_every_thread_its_agents_spawned_under_it_deepest_first() {
    local(async {
        let ctx = FakeThreadsCtx::new();
        ctx.sessions.with(|s| {
            *s = rows_of(vec![
                row(view(json!({ "id": "r-root", "threadId": "root", "status": "running" })), "u0"),
                row(view(json!({ "id": "r-child", "threadId": "child", "parentThreadId": "root", "status": "running" })), "u1"),
                row(view(json!({ "id": "r-grand", "threadId": "grand", "parentThreadId": "child", "status": "running" })), "u2"),
            ])
        });
        let area = ctx.area();
        let stopped = area.stop_under("root", None).await;
        assert_eq!(stopped, vec!["grand", "child"]);
        assert_eq!(
            ctx.called(),
            vec!["interrupt r-grand", "interrupt r-child"],
            "the root is the caller's own to stop, so only what spawned under it is asked here"
        );
    });
}

// TS: settle-tree.test.ts:156
#[test]
fn a_threads_own_token_settles_the_threads_under_it_and_is_refused_for_its_lead_and_a_thread_beside_it() {
    let ctx = FakeThreadsCtx::new();
    ctx.sessions.with(|s| {
        *s = rows_of(vec![
            row(view(json!({ "id": "r-l", "threadId": "L" })), "u0"),
            row(view(json!({ "id": "r-a", "threadId": "A", "parentThreadId": "L" })), "u1"),
            row(view(json!({ "id": "r-a1", "threadId": "A1", "parentThreadId": "A" })), "u2"),
            row(view(json!({ "id": "r-b", "threadId": "B", "parentThreadId": "L" })), "u3"),
            row(view(json!({ "id": "r-c", "threadId": "C", "parentThreadId": "L" })), "u4"),
        ])
    });
    let area = ctx.area();
    assert_eq!(area.tree_under("L"), vec!["A", "B", "C", "A1"], "level by level in the order the rows hold them");
    assert_eq!(area.tree_under("A"), vec!["A1"]);
    let as_a = scoped("A", "ws1", "L");
    assert!(area.settles_thread("A1", Some(&as_a)), "a thread under it");
    assert!(area.settles_thread("A", Some(&as_a)), "itself");
    assert!(!area.settles_thread("L", Some(&as_a)), "its lead's fold is the person's");
    assert!(!area.settles_thread("B", Some(&as_a)), "a thread beside it");
    for thread in ["L", "A", "A1", "B", "C"] {
        assert!(area.settles_thread(thread, None), "the person settles any thread: {thread}");
    }
}

// TS: waiting-behind.test.ts:109
#[test]
fn a_thread_inside_a_send_that_waits_for_another_thread_carries_that_threads_question() {
    let (_ctx, area) = waiting_pair();
    area.lead_asks.borrow_mut().insert("target01".into(), prompt_of("ask_1"));
    let caller_view = view(json!({ "id": "r-caller", "threadId": "caller01" }));
    let open = CallsOf {
        view: caller_view.clone(),
        calls: Some(calls_of(&[("toolu_send", "mcp__wsp__send", json!({ "thread": "target01", "message": "read hello.txt" }))])),
    };
    let behind = area.stopped_behind(&open).expect("the caller is stopped behind the question");
    assert_eq!(
        (behind.thread_id.as_str(), behind.workspace_id.as_str(), behind.session_id.as_str(), behind.title.as_str()),
        ("target01", "ws1", "r-target", "read the file")
    );
    assert_eq!((behind.prompt.ask_id.as_str(), behind.prompt.tool_name.as_str()), ("ask_1", "Read"));
    assert_eq!(behind.prompt.options.iter().map(|o| o.id.clone()).collect::<Vec<_>>(), vec!["allow", "deny"]);
    // The question is answered off the other thread: once it is, nothing is open and the caller works again.
    area.lead_asks.borrow_mut().shift_remove("target01");
    assert!(area.stopped_behind(&open).is_none());
}

// TS: waiting-behind.test.ts:140
#[test]
fn a_thread_is_behind_another_only_through_a_call_that_waits_for_it_and_never_a_detached_send() {
    let (_ctx, area) = waiting_pair();
    area.lead_asks.borrow_mut().insert("target01".into(), prompt_of("ask_1"));
    let caller_view = view(json!({ "id": "r-caller", "threadId": "caller01" }));
    let detached = CallsOf {
        view: caller_view.clone(),
        calls: Some(calls_of(&[("toolu_send", "mcp__wsp__send", json!({ "thread": "target01", "message": "go", "detach": true }))])),
    };
    assert!(area.stopped_behind(&detached).is_none(), "a detached send answers at once");
    let own =
        CallsOf { view: caller_view.clone(), calls: Some(calls_of(&[("toolu_read", "Read", json!({ "file_path": "/root/notes.md" }))])) };
    assert!(area.stopped_behind(&own).is_none(), "a call of the agent's own, into no thread at all, is no wait");
}

// TS: waiting-behind.test.ts:156
#[test]
fn the_wait_is_dropped_when_the_call_the_thread_was_inside_answers_whatever_the_other_thread_is_doing() {
    let (_ctx, area) = waiting_pair();
    area.lead_asks.borrow_mut().insert("target01".into(), prompt_of("ask_1"));
    let caller_view = view(json!({ "id": "r-caller", "threadId": "caller01" }));
    let waiting = CallsOf {
        view: caller_view.clone(),
        calls: Some(calls_of(&[("toolu_wait", "mcp__wsp__threads_wait", json!({ "threads": ["target01"], "timeout": 60 }))])),
    };
    assert_eq!(area.stopped_behind(&waiting).map(|b| b.thread_id), Some("target01".to_string()));
    // The wait gave up on its deadline: the call answered, so it is off the row while the question still stands.
    let answered = CallsOf { view: caller_view, calls: Some(IndexMap::new()) };
    assert!(area.stopped_behind(&answered).is_none());
    assert!(area.lead_asks.borrow().contains_key("target01"), "the thread that was asked still is");
}
