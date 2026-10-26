// SPDX-License-Identifier: AGPL-3.0-only
//! The threads area's tests ported from the TS suite (packages/runtime/test), at the area's edge: the store rows, the
//! events `record` was handed, the calls out in order, the warnings word for word. The cases the spec tables name are
//! in threads_{tree,kept,cap,owed,turn_end}.rs; these are the strong TS cases those files do not already cover.
//! Expected values read off threads.ts at 65f6b89c7 and the TS case named above each test.
#![allow(dead_code, unused_imports)]

#[path = "support_b/mod.rs"]
mod support;
include!("threads_ported/world.rs");

// =====================================================================================================================
// owed-lines.test.ts: a child's finished line its lead cannot take yet
// =====================================================================================================================

// TS: owed-lines.test.ts:214
#[test]
fn a_try_whose_launch_failed_silent_goes_again_under_the_same_request_and_the_next_try_takes_the_line() {
    local(async {
        let (ctx, area) = owed_host();
        let (h1, tx1) = handle("r-try", "u-try", SessionStartOutcome::Started, None);
        let (h2, _tx2) = handle("r-try2", "u-try2", SessionStartOutcome::Started, None);
        ctx.starts.borrow_mut().push_back(Ok(h1));
        ctx.starts.borrow_mut().push_back(Ok(h2));
        ctx.sessions.with(|s| {
            let r = row(
                view(json!({ "id": "r-try", "threadId": PARENT, "claudeSessionId": "cs-p", "status": "failed", "startedAt": 9 })),
                "u-try",
            );
            r.borrow_mut().turn_live =
                Some(TurnLive { failure: Some(silent("the machine did not answer", "LaunchUnanswered")), ..Default::default() });
            s.insert("r-try".into(), r);
        });
        area.deliver(line("L1"), false);
        settle().await;
        let warned = wsp_core::console::capture();
        finish(&tx1, result("failed", Some("the launch went unanswered")));
        settle().await;
        assert_eq!(
            warned.lines(),
            vec![
                "thread child000 ended, but its line did not reach thread parent00 yet, and goes again in 30 s: the launch went unanswered"
            ]
        );
        assert_eq!(ctx.started.borrow().len(), 1, "the next try waits its thirty seconds");
        ctx.clock.advance(30_000);
        settle().await;
        {
            let started = ctx.started.borrow();
            assert_eq!(started.len(), 2);
            assert_eq!(started[0].1.request_id.as_deref(), Some("uuid-1"));
            assert_eq!(started[1].1.request_id.as_deref(), Some("uuid-1"), "every try goes under the one request id");
        }
        taken_by(&ctx, "uuid-1", "r-try2", "u-try2");
        area.prompt_held("uuid-1");
        settle().await;
        assert_eq!(
            ctx.store.logged(),
            vec!["put notify-owed/L1", "put notify-owed/L1", "delete notify-owed/L1"],
            "kept before the first try, again with its spacing, and gone once taken"
        );
        assert!(ctx.called().contains(&format!("mark {FROM} 1030000")));
        assert!(told_person(&ctx.recorded_json()).is_empty());
        assert!(area.sending.borrow().is_empty());
    });
}

// TS: owed-lines.test.ts:233
#[test]
fn a_try_that_fails_after_the_leads_agent_announced_itself_is_not_sent_again() {
    local(async {
        let (ctx, area) = owed_host();
        let (h, tx) = handle("r-try", "u-try", SessionStartOutcome::Started, None);
        ctx.starts.borrow_mut().push_back(Ok(h));
        area.deliver(line("L1"), false);
        settle().await;
        let warned = wsp_core::console::capture();
        taken_by(&ctx, "uuid-1", "r-try", "u-try");
        finish(&tx, result("failed", Some("the launch was lost")));
        settle().await;
        ctx.clock.advance(2 * HOUR);
        settle().await;
        assert!(warned.lines().is_empty(), "the agent had the line, so nothing failed");
        assert_eq!(ctx.started.borrow().len(), 1);
        assert_eq!(ctx.store.logged(), vec!["put notify-owed/L1", "delete notify-owed/L1"]);
        assert!(told_person(&ctx.recorded_json()).is_empty());
        assert!(ctx.called().contains(&format!("mark {FROM} 1000000")), "a lead that took a child's line has read the child");
    });
}

// TS: owed-lines.test.ts:268
#[test]
fn a_try_whose_agent_had_not_announced_itself_at_the_restart_is_taken_when_it_does() {
    local(async {
        let (ctx, area) = owed_host();
        ctx.sessions.with(|s| {
            let r = row(
                view(json!({ "id": "r-dark", "threadId": PARENT, "claudeSessionId": "cs-p", "status": "running", "startedAt": 5 })),
                "u-dark",
            );
            r.borrow_mut().asked = Some(TurnAsked {
                prompt: "x".into(),
                effort: None,
                typed: None,
                request_id: Some("req-d".into()),
                owed: Some(true),
                awaits_prompt: None,
            });
            s.insert("r-dark".into(), r);
        });
        let mut l = line("L1");
        l.since = Some(1_000_000 - 60_000);
        l.request_id = Some("req-d".into());
        let warned = wsp_core::console::capture();
        area.deliver(l, false);
        settle().await;
        assert!(ctx.started.borrow().is_empty(), "the try out is read, not sent again");
        assert!(ctx.store.logged().is_empty());
        taken_by(&ctx, "req-d", "r-dark", "u-dark");
        area.prompt_held("req-d");
        settle().await;
        assert_eq!(ctx.store.logged(), vec!["delete notify-owed/L1"]);
        ctx.clock.advance(2 * HOUR);
        settle().await;
        assert!(!ctx.called().iter().any(|c| c.starts_with("stopTry")), "nothing reaches a dark turn to stop it");
        assert!(ctx.started.borrow().is_empty());
        assert!(warned.lines().is_empty());
        assert!(told_person(&ctx.recorded_json()).is_empty());
        assert!(ctx.called().contains(&format!("mark {FROM} 1000000")));
    });
}

// TS: owed-lines.test.ts:286
#[test]
fn a_line_still_owed_at_the_restart_is_sent_again_under_its_request_by_the_next_host() {
    local(async {
        let (ctx, area) = owed_host();
        let owed = Owed {
            since: Some(1_000_000 - 300_000),
            request_id: Some("req-k".into()),
            tries: Some(1),
            next: Some(1_000_000 - 1_000),
            ..line("L1")
        };
        ctx.store.seed("notify-owed", "L1", serde_json::to_value(&owed).expect("a line"));
        let (h, _tx) = handle("r-try", "u-try", SessionStartOutcome::Started, None);
        ctx.starts.borrow_mut().push_back(Ok(h));
        area.deliver_owed().await.expect("listed");
        settle().await;
        {
            let started = ctx.started.borrow();
            assert_eq!(started.len(), 1, "its next time is past, so it goes now");
            assert_eq!(started[0].1.request_id.as_deref(), Some("req-k"));
            assert_eq!(started[0].1.prompt, "thread child000 finished (completed): the report");
        }
        assert!(ctx.store.logged().is_empty(), "a line with its request and its start is not written again");
        taken_by(&ctx, "req-k", "r-try", "u-try");
        area.prompt_held("req-k");
        settle().await;
        assert_eq!(ctx.store.logged(), vec!["delete notify-owed/L1"]);
        assert!(area.sending.borrow().is_empty());
        assert!(told_person(&ctx.recorded_json()).is_empty());
    });
}

// TS: owed-lines.test.ts:367, 389
#[test]
fn a_steer_whose_write_outlasts_the_leads_turn_goes_as_its_next_turn_or_tells_the_person_once() {
    for stopped in [false, true] {
        local(async move {
            let (ctx, area) = owed_host();
            ctx.sessions.with(|s| {
                let r = row(view(json!({ "id": "r1", "threadId": "t1", "claudeSessionId": "cs-1", "status": "running" })), "u1");
                r.borrow_mut().turn_live = Some(TurnLive::default());
                s.insert("r1".into(), r);
            });
            area.keep_steer("r1", &steer_sent("also this"), None, "st1").await;
            // The turn ends with the message unread while its write is still out: the start road owns it.
            let ended = turn_row(&ctx, "r1");
            area.send_back(&ended, &["st1".to_string()], stopped);
            settle().await;
            assert!(ctx.started.borrow().is_empty(), "the turn's end does not send a message whose write is out");
            assert!(ctx.recorded.borrow().is_empty());
            assert!(ctx.store.logged().is_empty());
            let landed = area.steer_answered("r1", "h1", &steer_sent("also this"), None, "st1", true);
            settle().await;
            assert!(!area.steers_out.borrow().contains_key("st1"));
            if stopped {
                assert!(landed, "the person is told, so the caller hears it landed");
                let told = ctx.recorded_json();
                assert_eq!(told.len(), 1);
                assert_eq!(
                    (told[0]["type"].clone(), told[0]["notify"].clone(), told[0]["text"].clone()),
                    (json!("session.notify"), json!("me"), json!("the agent never read this message: also this"))
                );
            } else {
                assert!(!landed, "the start road sends it as the thread's next turn");
                assert!(steered_on(&ctx).is_empty(), "the turn keeps no message it will not read");
                assert!(ctx.recorded.borrow().is_empty(), "and nobody is told it went wrong");
            }
            assert!(ctx.started.borrow().is_empty());
        });
    }
}

// TS: owed-lines.test.ts:442
#[test]
fn a_try_whose_row_ends_is_read_off_that_end_as_stopped_or_failed() {
    for (status, reason, warning) in [
        ("interrupted", "stopped", "thread child000 ended, but the turn carrying its line into thread parent00 was stopped"),
        ("failed", "the stream broke", "thread child000 ended, but its line did not reach thread parent00: the stream broke"),
    ] {
        local(async move {
            let (ctx, area) = owed_host();
            let (h, _tx) = handle("r-try", "u-try", SessionStartOutcome::Started, None);
            ctx.starts.borrow_mut().push_back(Ok(h));
            ctx.sessions.with(|s| {
                s.insert("r-try".into(), row(view(json!({ "id": "r-try", "status": status })), "u-try"));
            });
            area.deliver(line("L1"), false);
            settle().await;
            let warned = wsp_core::console::capture();
            ctx.bus.emit_json(json!({ "type": "session.end", "workspaceId": "ws1", "sessionId": "cs", "turnId": "u-try", "exitCode": null, "sawResult": false, "reason": reason }));
            settle().await;
            assert_eq!(warned.lines(), vec![warning]);
            assert!(ctx.store.get_now("notify-owed", "L1").is_none());
            assert_eq!(told_person(&ctx.recorded_json()).len(), 1, "a line that fell tells the person once");
            assert_eq!(ctx.started.borrow().len(), 1);
        });
    }
}

// TS: owed-lines.test.ts:458
#[test]
fn a_line_the_leads_computer_keeps_failing_tells_the_person_once_and_the_lead_nothing() {
    local(async {
        let (ctx, area) = owed_host();
        for _ in 0..12 {
            ctx.starts.borrow_mut().push_back(Err(silent("the computer did not answer", "MachineUnreached")));
        }
        area.deliver(line("L1"), false);
        settle().await;
        for w in [30, 60, 120, 240, 480, 600, 600, 600, 600, 270] {
            ctx.clock.advance(w * 1000);
            settle().await;
        }
        assert!(ctx.started.borrow().len() > 5, "the line was tried many times");
        let said = ctx.recorded_json();
        assert_eq!(said.len(), 1, "no try tells the lead anything: the person gets the one row, at the hour");
        assert_eq!(said[0]["type"], "session.notify");
        assert_eq!(said[0]["notify"], "me");
        assert_eq!(
            (said[0]["workspaceId"].clone(), said[0]["sessionId"].clone(), said[0]["turnId"].clone(), said[0]["threadId"].clone()),
            (json!("ws-c"), json!("cs-c"), json!("u-c"), json!(FROM))
        );
        assert_eq!(said[0]["text"], "thread child000 finished (completed)");
        assert!(ctx.store.get_now("notify-owed", "L1").is_none());
    });
}

// TS: owed-lines.test.ts:591
#[test]
fn the_persons_stop_of_a_turn_nothing_reaches_settles_its_line_without_stopping_the_turn() {
    local(async {
        let (ctx, area) = owed_host();
        ctx.sessions.with(|s| {
            let r = row(
                view(json!({ "id": "r-dark", "threadId": PARENT, "claudeSessionId": "cs-p", "status": "running", "startedAt": 5 })),
                "u-dark",
            );
            r.borrow_mut().asked = Some(TurnAsked {
                prompt: "x".into(),
                effort: None,
                typed: None,
                request_id: Some("req-d".into()),
                owed: Some(true),
                awaits_prompt: None,
            });
            s.insert("r-dark".into(), r);
        });
        let mut l = line("L1");
        l.since = Some(1_000_000 - 60_000);
        l.request_id = Some("req-d".into());
        area.deliver(l, false);
        settle().await;
        let warned = wsp_core::console::capture();
        area.try_stopped("u-dark");
        settle().await;
        assert_eq!(warned.lines(), vec!["thread child000 ended, but the turn carrying its line into thread parent00 was stopped"]);
        assert!(!ctx.called().iter().any(|c| c.starts_with("stopTry")), "nothing reaches the turn itself");
        assert_eq!(ctx.store.logged(), vec!["delete notify-owed/L1"]);
        assert_eq!(told_person(&ctx.recorded_json()).len(), 1);
        ctx.clock.advance(2 * HOUR);
        settle().await;
        assert!(ctx.started.borrow().is_empty());
        assert_eq!(told_person(&ctx.recorded_json()).len(), 1);
    });
}

// TS: owed-lines.test.ts:661
#[test]
fn a_steered_line_whose_turn_ended_with_no_word_on_unread_messages_falls_to_the_person() {
    local(async {
        let (ctx, area) = owed_host();
        let (h, tx) = handle("r-run", "u-run", SessionStartOutcome::Started, None);
        ctx.sessions.with(|s| {
            let r = row(
                view(json!({ "id": "r-run", "threadId": PARENT, "claudeSessionId": "cs-p", "status": "running", "startedAt": 5 })),
                "u-run",
            );
            let steered: Steered =
                serde_json::from_value(json!({ "prompt": "x", "startedBy": "agent", "requestId": "req-s" })).expect("a steer");
            r.borrow_mut().turn_live =
                Some(TurnLive { steered: Some([("st1".to_string(), steered)].into_iter().collect()), ..Default::default() });
            r.borrow_mut().handle = Some(h);
            s.insert("r-run".into(), r);
        });
        finish(&tx, result("completed", None));
        let warned = wsp_core::console::capture();
        let mut l = line("L1");
        l.since = Some(1_000_000);
        l.request_id = Some("req-s".into());
        area.deliver(l, false);
        settle().await;
        assert_eq!(warned.lines(), vec!["thread child000 ended, and nothing says whether thread parent00 read its line"]);
        assert!(ctx.started.borrow().is_empty());
        assert_eq!(ctx.store.logged(), vec!["delete notify-owed/L1"]);
        assert_eq!(told_person(&ctx.recorded_json()).len(), 1, "told, as a client with no unread report was, and never lost");
    });
}

// TS: owed-lines.test.ts:531
#[test]
fn a_steered_line_on_a_turn_no_handle_reaches_is_asked_again_every_thirty_seconds_until_the_turn_is_over() {
    local(async {
        let (ctx, area) = owed_host();
        ctx.sessions.with(|s| {
            let r = row(
                view(json!({ "id": "r-run", "threadId": PARENT, "claudeSessionId": "cs-p", "status": "running", "startedAt": 5 })),
                "u-run",
            );
            let steered: Steered =
                serde_json::from_value(json!({ "prompt": "x", "startedBy": "agent", "requestId": "req-s" })).expect("a steer");
            r.borrow_mut().turn_live =
                Some(TurnLive { steered: Some([("st1".to_string(), steered)].into_iter().collect()), ..Default::default() });
            s.insert("r-run".into(), r);
        });
        let (h, _tx) = handle("r-try", "u-try", SessionStartOutcome::Started, None);
        ctx.starts.borrow_mut().push_back(Ok(h));
        let warned = wsp_core::console::capture();
        let mut l = line("L1");
        l.since = Some(1_000_000);
        l.request_id = Some("req-s".into());
        area.deliver(l, false);
        settle().await;
        assert!(ctx.started.borrow().is_empty());
        ctx.clock.advance(29_999);
        settle().await;
        assert!(ctx.started.borrow().is_empty(), "the wait on a turn nothing answers for is thirty seconds on the clock");
        ctx.clock.advance(1);
        settle().await;
        assert!(ctx.started.borrow().is_empty(), "the turn still runs, so the line asks again");
        assert!(area.sending.borrow().contains_key("L1"));
        ctx.sessions.with(|s| s.shift_remove("r-run"));
        ctx.clock.advance(30_000);
        settle().await;
        let started = ctx.started.borrow();
        assert_eq!(started.len(), 1, "once the turn is over the line goes under its request");
        assert_eq!(started[0].1.request_id.as_deref(), Some("req-s"));
        assert!(warned.lines().is_empty());
        assert!(ctx.store.logged().is_empty(), "a line that had its request and start is never written for waiting");
    });
}

// TS: owed-lines.test.ts:683
#[test]
fn a_line_whose_try_failed_and_whose_lead_napped_past_its_hour_is_tried_at_the_wake() {
    local(async {
        let (ctx, area) = owed_host();
        set_phase(&ctx, "ws1", "napping");
        let warned = wsp_core::console::capture();
        let mut l = line("L1");
        l.since = Some(1_000_000 - 2 * HOUR);
        l.request_id = Some("req-n".into());
        l.tries = Some(1);
        l.next = Some(1_000_000 - HOUR);
        area.deliver(l, false);
        settle().await;
        assert_eq!(area.held_lines.borrow().get("ws1").map(|l| l.len()), Some(1), "held for the wake, past its hour or not");
        assert!(ctx.started.borrow().is_empty());
        assert!(warned.lines().is_empty(), "the hour bounds tries while the computer answers: nothing gives up");
        set_phase(&ctx, "ws1", "running");
        let (h, _tx) = handle("r-try", "u-try", SessionStartOutcome::Started, None);
        ctx.starts.borrow_mut().push_back(Ok(h));
        ctx.bus.emit_json(woken("ws1"));
        settle().await;
        {
            let started = ctx.started.borrow();
            assert_eq!(started.len(), 1, "the wake gets a try");
            assert_eq!(started[0].1.request_id.as_deref(), Some("req-n"));
            assert!(!started[0].1.asks_until_stopped, "its hour is gone, so it does not ask until stopped");
            assert!(started[0].1.wakes_lead);
        }
        taken_by(&ctx, "req-n", "r-try", "u-try");
        area.prompt_held("req-n");
        settle().await;
        assert_eq!(ctx.store.logged(), vec!["delete notify-owed/L1"]);
        assert!(warned.lines().is_empty());
        assert!(told_person(&ctx.recorded_json()).is_empty());
    });
}

// TS: owed-lines.test.ts:817, 830
#[test]
fn a_try_that_ends_before_its_agent_started_leaves_the_line_to_the_person_with_the_turns_words() {
    for (error, said) in [
        (None, "the turn ended failed before its agent started"),
        (Some("the model is not available on this plan"), "the model is not available on this plan"),
    ] {
        local(async move {
            let (ctx, area) = owed_host();
            let (h, tx) = handle("r-try", "u-try", SessionStartOutcome::Started, None);
            ctx.starts.borrow_mut().push_back(Ok(h));
            area.deliver(line("L1"), false);
            settle().await;
            let warned = wsp_core::console::capture();
            finish(&tx, result("failed", error));
            settle().await;
            ctx.clock.advance(2 * HOUR);
            settle().await;
            assert_eq!(warned.lines(), vec![format!("thread child000 ended, but its line did not reach thread parent00: {said}")]);
            assert_eq!(told_person(&ctx.recorded_json()).len(), 1);
            assert_eq!(ctx.started.borrow().len(), 1, "never tried again");
            assert!(ctx.store.get_now("notify-owed", "L1").is_none());
        });
    }
}

// TS: owed-lines.test.ts:1044
#[test]
fn a_next_host_reads_the_try_only_once_its_reopen_answered_whether_it_comes_up_within_the_hour_or_past_it() {
    for after_minutes in [61_i64, 30] {
        local(async move {
            let (ctx, area) = owed_host();
            let (tx, rx) = futures::channel::oneshot::channel::<()>();
            let gate: Joined = rx.map(|_| ()).boxed_local().shared();
            ctx.boot_work.borrow_mut().insert("ws1".into(), gate);
            let (h, _htx) = handle("r-run", "u-run", SessionStartOutcome::Started, None);
            ctx.sessions.with(|s| {
                let r = row(
                    view(json!({ "id": "r-run", "threadId": PARENT, "claudeSessionId": "cs-p", "status": "running", "startedAt": 5 })),
                    "u-run",
                );
                r.borrow_mut().asked = Some(TurnAsked {
                    prompt: "x".into(),
                    effort: None,
                    typed: None,
                    request_id: Some("req-w".into()),
                    owed: Some(true),
                    awaits_prompt: None,
                });
                r.borrow_mut().handle = Some(h);
                s.insert("r-run".into(), r);
            });
            let warned = wsp_core::console::capture();
            let mut l = line("L1");
            l.since = Some(1_000_000 - after_minutes * 60_000);
            l.request_id = Some("req-w".into());
            area.deliver(l, false);
            settle().await;
            assert!(ctx.store.logged().is_empty(), "nothing is read before the re-open answered");
            assert!(ctx.started.borrow().is_empty());
            assert!(!ctx.called().iter().any(|c| c.starts_with("stopTry")));
            assert!(warned.lines().is_empty());
            // The agent took the line while the host was down, so the re-open finds it taken.
            taken_by(&ctx, "req-w", "r-run", "u-run");
            let _ = tx.send(());
            settle().await;
            assert_eq!(ctx.store.logged(), vec!["delete notify-owed/L1"], "the lead took the line: nobody is told it failed");
            assert!(warned.lines().is_empty());
            assert!(told_person(&ctx.recorded_json()).is_empty());
            assert!(ctx.started.borrow().is_empty());
        });
    }
}

// =====================================================================================================================
// runtime-notify.test.ts: a thread whose start named who to tell
// =====================================================================================================================

// TS: runtime-notify.test.ts:10
#[test]
fn a_running_parent_is_told_by_one_steer_the_line_with_outcome_duration_cost_and_the_reply_whole() {
    local(async {
        let (ctx, area) = owed_host();
        let (h, _tx) = handle("r-p", "u-p2", SessionStartOutcome::Steered, None);
        ctx.starts.borrow_mut().push_back(Ok(h));
        let done = result_of(
            json!({ "status": "completed", "text": "Ran the gate.\n\nAll 12 tests green.\n", "durationMs": 492_000, "costUsd": 1.94 }),
        );
        area.notify_end(&child_row(Some("cs-c"), None), &[PARENT.to_string()], &TellAs::default(), &done, false);
        settle().await;
        let line = "thread child000 finished (completed, 8m 12s, $1.94): Ran the gate.\n\nAll 12 tests green.";
        let said = ctx.recorded_json();
        assert_eq!(said.len(), 1);
        for (k, v) in [
            ("type", json!("session.notify")),
            ("workspaceId", json!("ws-c")),
            ("sessionId", json!("cs-c")),
            ("turnId", json!("u-c")),
            ("threadId", json!(FROM)),
            ("notify", json!(PARENT)),
            ("text", json!(line)),
        ] {
            assert_eq!(said[0][k], v, "{k}");
        }
        {
            let started = ctx.started.borrow();
            assert_eq!(started.len(), 1);
            assert_eq!(started[0].1.prompt, line);
            assert_eq!((started[0].1.harness.as_deref(), started[0].1.thread.as_deref()), (Some("claude"), Some(PARENT)));
            assert!(started[0].1.wakes_lead && started[0].1.owed);
        }
        assert_eq!(ctx.store.logged(), vec![format!("put notify-owed/u-c:0:{PARENT}"), format!("delete notify-owed/u-c:0:{PARENT}")]);
        assert!(ctx.called().contains(&format!("mark {FROM} 1000000")));
    });
}

// TS: runtime-notify.test.ts:32
#[test]
fn an_idle_parent_is_told_by_a_turn_of_its_own_with_the_line_as_its_prompt() {
    local(async {
        let (ctx, area) = owed_host();
        let (h, _tx) = handle("r-n", "u-n", SessionStartOutcome::Started, None);
        ctx.starts.borrow_mut().push_back(Ok(h));
        let done = result_of(json!({ "status": "completed", "text": "done", "durationMs": 1_500, "costUsd": 0.0042 }));
        area.notify_end(&child_row(Some("cs-c"), None), &[PARENT.to_string()], &TellAs::default(), &done, false);
        settle().await;
        let line = "thread child000 finished (completed, 1.5s, $0.00): done";
        assert_eq!(ctx.started.borrow().len(), 1);
        assert_eq!(ctx.started.borrow()[0].1.prompt, line);
        let kept = ctx.store.get_now("notify-owed", &format!("u-c:0:{PARENT}")).expect("owed until the turn takes it");
        assert_eq!(kept["text"], json!(line));
        assert_eq!(kept["toPerson"]["text"], json!(line));
        taken_by(&ctx, "uuid-1", "r-n", "u-n");
        area.prompt_held("uuid-1");
        settle().await;
        assert!(ctx.store.get_now("notify-owed", &format!("u-c:0:{PARENT}")).is_none());
    });
}

// TS: runtime-notify.test.ts:57
#[test]
fn a_childs_line_taken_by_its_lead_marks_the_child_read_by_steer_or_by_a_turn_of_the_leads_own() {
    for steers in [true, false] {
        local(async move {
            let (ctx, area) = owed_host();
            let outcome = if steers { SessionStartOutcome::Steered } else { SessionStartOutcome::Started };
            let (h, _tx) = handle("r-n", "u-n", outcome, None);
            ctx.starts.borrow_mut().push_back(Ok(h));
            area.notify_end(&child_row(Some("cs-c"), None), &[PARENT.to_string()], &TellAs::default(), &result("completed", None), false);
            ctx.clock.advance(5_000);
            settle().await;
            if !steers {
                assert!(!ctx.called().iter().any(|c| c.starts_with("mark")), "not read until a turn of the lead took the line");
                taken_by(&ctx, "uuid-1", "r-n", "u-n");
                area.prompt_held("uuid-1");
                settle().await;
            }
            let marks: Vec<String> = ctx.called().into_iter().filter(|c| c.starts_with("mark")).collect();
            assert_eq!(
                marks,
                vec![format!("mark {FROM} 1005000")],
                "the child is read as of the moment its line was taken (steers: {steers})"
            );
        });
    }
}

// TS: runtime-notify.test.ts:82, 196
#[test]
fn a_line_queued_behind_the_parents_running_process_stays_owed_and_goes_once_as_a_turn_of_its_own() {
    local(async {
        let (ctx, area) = owed_host();
        let (h, _tx) = handle("r-q", "u-q", SessionStartOutcome::Queued, None);
        ctx.starts.borrow_mut().push_back(Ok(h));
        area.notify_end(
            &child_row(Some("cs-c"), None),
            &[PARENT.to_string()],
            &TellAs::default(),
            &result_of(json!({ "status": "completed", "text": "done" })),
            false,
        );
        settle().await;
        let said = ctx.recorded_json();
        assert_eq!(said.len(), 1);
        assert_eq!(
            (said[0]["notify"].clone(), said[0]["text"].clone()),
            (json!(PARENT), json!("thread child000 finished (completed): done"))
        );
        assert_eq!(ctx.started.borrow().len(), 1, "queued behind its process, as any send is, and never steered");
        let key = format!("u-c:0:{PARENT}");
        assert!(ctx.store.get_now("notify-owed", &key).is_some(), "owed until the queued turn takes it");
        assert_eq!(ctx.store.logged(), vec![format!("put notify-owed/{key}")]);
        taken_by(&ctx, "uuid-1", "r-q", "u-q");
        area.prompt_held("uuid-1");
        settle().await;
        assert_eq!(ctx.store.logged(), vec![format!("put notify-owed/{key}"), format!("delete notify-owed/{key}")]);
        assert_eq!(ctx.started.borrow().len(), 1);
    });
}

// TS: runtime-notify.test.ts:217
#[test]
fn failed_and_interrupted_ends_carry_their_words_and_me_starts_nothing() {
    let (ctx, area) = owed_host();
    let failing = row(view(json!({ "id": "r-1", "workspaceId": "ws-c", "threadId": "failing01", "claudeSessionId": "cs-1" })), "u1");
    let stopped = row(view(json!({ "id": "r-2", "workspaceId": "ws-c", "threadId": "stopped01", "claudeSessionId": "cs-2" })), "u2");
    area.notify_end(
        &failing,
        &["me".to_string()],
        &TellAs::default(),
        &result_of(json!({ "status": "failed", "error": "the harness died", "durationMs": 3_000 })),
        false,
    );
    area.notify_end(
        &stopped,
        &["me".to_string()],
        &TellAs::default(),
        &result_of(json!({ "status": "interrupted", "text": "Stopped mid-way.", "durationMs": 12_000, "costUsd": 0.03 })),
        false,
    );
    let said = ctx.recorded_json();
    assert_eq!(said.len(), 2);
    assert_eq!(
        (said[0]["threadId"].clone(), said[0]["notify"].clone(), said[0]["text"].clone()),
        (json!("failing01"), json!("me"), json!("thread failing0 finished (failed, 3.0s): the harness died"))
    );
    assert_eq!(
        (said[1]["threadId"].clone(), said[1]["notify"].clone(), said[1]["text"].clone()),
        (json!("stopped01"), json!("me"), json!("thread stopped0 finished (interrupted, 12s, $0.03): Stopped mid-way."))
    );
    assert!(ctx.started.borrow().is_empty());
    assert!(ctx.store.logged().is_empty(), "me is the person's row alone: nothing is kept owed");
}

// TS: runtime-notify.test.ts:296
#[test]
fn a_notify_that_leads_back_to_a_thread_shows_in_its_reach_at_any_length_of_the_chain() {
    let ctx = FakeThreadsCtx::new();
    ctx.sessions.with(|s| {
        let a = row(view(json!({ "id": "ra", "threadId": "a" })), "ua");
        let b = row(view(json!({ "id": "rb", "threadId": "b" })), "ub");
        b.borrow_mut().notify = Some(vec!["a".into()]);
        let c = row(view(json!({ "id": "rc", "threadId": "c" })), "uc");
        c.borrow_mut().notify = Some(vec!["b".into()]);
        *s = rows_of(vec![a, b, c]);
    });
    let area = ctx.area();
    let reach = |from: &[&str]| area.notify_reach(&from.iter().map(|s| s.to_string()).collect::<Vec<_>>()).into_iter().collect::<Vec<_>>();
    assert_eq!(reach(&["b"]), vec!["b", "a"], "a start into a naming b would loop: b already notifies a");
    assert_eq!(reach(&["c"]), vec!["c", "b", "a"], "and so would one naming c, a chain of three");
    assert_eq!(reach(&["a"]), vec!["a"], "a chain that does not come back is fine: a fresh thread naming c is in no one's reach");
    assert!(!reach(&["c"]).contains(&"d".to_string()));
}

// TS: runtime-notify.test.ts:341
#[test]
fn a_line_held_for_a_napping_parent_outlives_a_host_restart_and_goes_when_the_workspace_wakes() {
    local(async {
        let (ctx, area) = owed_host();
        set_phase(&ctx, "ws1", "napping");
        let owed = Owed { since: Some(1_000_000 - 30_000), request_id: Some("req-h".into()), ..line("L1") };
        ctx.store.seed("notify-owed", "L1", serde_json::to_value(&owed).expect("a line"));
        area.deliver_owed().await.expect("listed");
        settle().await;
        assert!(ctx.started.borrow().is_empty(), "the next host holds it again for the wake");
        assert_eq!(area.held_lines.borrow().get("ws1").map(|l| l.len()), Some(1));
        assert!(area.sending.borrow().contains_key("L1"));
        assert!(ctx.store.get_now("notify-owed", "L1").is_some());
        set_phase(&ctx, "ws1", "running");
        let (h, _tx) = handle("r-n", "u-n", SessionStartOutcome::Started, None);
        ctx.starts.borrow_mut().push_back(Ok(h));
        ctx.bus.emit_json(woken("ws1"));
        settle().await;
        {
            let started = ctx.started.borrow();
            assert_eq!(started.len(), 1);
            assert_eq!(started[0].1.request_id.as_deref(), Some("req-h"));
        }
        assert!(area.held_lines.borrow().get("ws1").is_none());
    });
}

// TS: runtime-notify.test.ts:370
#[test]
fn a_line_is_held_for_every_phase_that_cannot_take_a_start_and_sent_at_once_for_the_rest() {
    local(async {
        for (phase, held) in [
            ("pausing", true),
            ("napping", true),
            ("waking", true),
            ("gone", true),
            ("released", true),
            ("queued", false),
            ("running", false),
        ] {
            let (ctx, area) = owed_host();
            set_phase(&ctx, "ws1", phase);
            let (h, _tx) = handle("r-n", "u-n", SessionStartOutcome::Started, None);
            ctx.starts.borrow_mut().push_back(Ok(h));
            area.deliver(line("L1"), false);
            settle().await;
            assert_eq!(area.held_lines.borrow().get("ws1").map(|l| l.len()), if held { Some(1) } else { None }, "{phase}");
            assert_eq!(ctx.started.borrow().len(), usize::from(!held), "{phase}");
        }
        // A workspace that is not live is not held: the start decides.
        let (ctx, area) = owed_host();
        ctx.entries.borrow_mut().shift_remove("ws1");
        let (h, _tx) = handle("r-n", "u-n", SessionStartOutcome::Started, None);
        ctx.starts.borrow_mut().push_back(Ok(h));
        area.deliver(line("L1"), false);
        settle().await;
        assert!(area.held_lines.borrow().is_empty());
        assert_eq!(ctx.started.borrow().len(), 1);
    });
}

// TS: runtime-notify.test.ts:395
#[test]
fn a_turn_the_runtime_ends_is_told_to_its_target_as_failed_with_the_reason() {
    let (ctx, area) = owed_host();
    running_row(&ctx, "t1", Some(vec!["me"]));
    area.settle_cut(
        "r1",
        "machine paused while the agent was working",
        &|_| "machine paused while the agent was working".into(),
        false,
        false,
    );
    let said = ctx.recorded_json();
    assert_eq!(
        said.iter().map(|e| e["type"].clone()).collect::<Vec<_>>(),
        vec![json!("session.notify"), json!("session.end")],
        "the line goes before the end, where a follower stops reading"
    );
    assert_eq!(
        (said[0]["notify"].clone(), said[0]["text"].clone()),
        (json!("me"), json!("thread t1 finished (failed): machine paused while the agent was working"))
    );
    assert_eq!(said[1]["reason"], json!("machine paused while the agent was working"));
    assert_eq!(said[1]["sawResult"], json!(false));
    assert_eq!(ctx.sessions.with(|s| support::json_of(&s.get("r1").expect("row").borrow().view.status)), json!("failed"));
}

// TS: runtime-notify.test.ts:409
#[test]
fn a_turn_a_host_restart_cut_tells_its_parent_in_another_workspace_the_same_way() {
    local(async {
        let (ctx, area) = owed_host();
        let (h, _tx) = handle("r-n", "u-n", SessionStartOutcome::Started, None);
        ctx.starts.borrow_mut().push_back(Ok(h));
        ctx.sessions.with(|s| {
            let r = row(view(json!({ "id": "r-kid", "workspaceId": "ws-kid", "threadId": "kid00001", "claudeSessionId": "cs-kid", "status": "running" })), "u-kid");
            r.borrow_mut().notify = Some(vec![PARENT.into()]);
            s.insert("r-kid".into(), r);
        });
        area.settle_cut(
            "r-kid",
            "host restarted while the agent was working",
            &|_| "cut by a host restart after 12m 03s".into(),
            false,
            false,
        );
        settle().await;
        let line = "thread kid00001 finished (failed): cut by a host restart after 12m 03s";
        let said = ctx.recorded_json();
        assert_eq!(said.iter().map(|e| e["type"].clone()).collect::<Vec<_>>(), vec![json!("session.notify"), json!("session.end")]);
        for (k, v) in [
            ("workspaceId", json!("ws-kid")),
            ("sessionId", json!("cs-kid")),
            ("turnId", json!("u-kid")),
            ("threadId", json!("kid00001")),
            ("notify", json!(PARENT)),
            ("text", json!(line)),
        ] {
            assert_eq!(said[0][k], v, "{k}");
        }
        assert_eq!(said[1]["reason"], json!("host restarted while the agent was working"));
        let started = ctx.started.borrow();
        assert_eq!(started.len(), 1);
        assert_eq!((started[0].0.as_str(), started[0].1.prompt.as_str(), started[0].1.thread.as_deref()), ("ws1", line, Some(PARENT)));
    });
}

// TS: runtime-notify.test.ts:449
#[test]
fn a_turn_a_host_restart_cut_that_was_to_tell_me_records_the_line_in_its_thread() {
    let (ctx, area) = owed_host();
    running_row(&ctx, "t1", Some(vec!["me"]));
    area.settle_cut("r1", "host restarted while the agent was working", &|_| "cut by a host restart after 0m 45s".into(), false, false);
    let said = ctx.recorded_json();
    assert_eq!(said.iter().map(|e| e["type"].clone()).collect::<Vec<_>>(), vec![json!("session.notify"), json!("session.end")]);
    assert_eq!(
        (said[0]["turnId"].clone(), said[0]["notify"].clone(), said[0]["text"].clone()),
        (json!("u1"), json!("me"), json!("thread t1 finished (failed): cut by a host restart after 0m 45s"))
    );
    assert_eq!(said[1]["reason"], json!("host restarted while the agent was working"));
    assert!(ctx.started.borrow().is_empty());
    assert!(ctx.store.logged().is_empty());
}

// TS: runtime-notify.test.ts:474
#[test]
fn two_turns_each_carry_a_token_and_each_token_names_its_own_thread() {
    let ctx = FakeThreadsCtx::new();
    ctx.sessions.with(|s| {
        let one = row(view(json!({ "id": "a", "threadId": "thread-one", "status": "running" })), "u1");
        one.borrow_mut().turn_token = Some("0123456789abcdef0123456789abcdef".into());
        let two = row(view(json!({ "id": "b", "threadId": "thread-two", "status": "running" })), "u2");
        two.borrow_mut().turn_token = Some("fedcba9876543210fedcba9876543210".into());
        *s = rows_of(vec![one, two]);
    });
    let area = ctx.area();
    assert_eq!(area.thread_of_token("0123456789abcdef0123456789abcdef").expect("the first"), "thread-one");
    assert_eq!(area.thread_of_token("fedcba9876543210fedcba9876543210").expect("the second"), "thread-two");
}

// TS: runtime-notify.test.ts:500
#[test]
fn a_turn_the_runtime_ended_rather_than_its_own_process_leaves_its_token_naming_nobody() {
    let (ctx, area) = owed_host();
    running_row(&ctx, "t1", None);
    ctx.sessions.with(|s| s.get("r1").expect("row").borrow_mut().turn_token = Some("tok-nap".into()));
    assert_eq!(area.thread_of_token("tok-nap").expect("a running turn's token names its thread"), "t1");
    area.settle_cut("r1", "machine paused while the agent was working", &|_| "machine paused".into(), false, false);
    let refused = area.thread_of_token("tok-nap").expect_err("the nap ended the turn from this side");
    assert_eq!(refused.message, NO_SUCH_TURN);
    assert_eq!(refused.kind, None);
}

// TS: runtime-notify.test.ts:542
#[test]
fn several_targets_are_each_told_once_with_the_same_whole_line() {
    local(async {
        let (ctx, area) = owed_host();
        ctx.sessions.with(|s| {
            s.insert("r-p2".into(), row(view(json!({ "id": "r-p2", "threadId": "parent0002", "claudeSessionId": "cs-p2", "status": "completed", "startedAt": 2 })), "u-p2"));
        });
        for n in 0..2 {
            let (h, _tx) = handle(&format!("r-n{n}"), &format!("u-n{n}"), SessionStartOutcome::Steered, None);
            ctx.starts.borrow_mut().push_back(Ok(h));
        }
        let done = result_of(json!({ "status": "completed", "text": "Ran the gate.\nAll 12 tests green." }));
        area.notify_end(&child_row(Some("cs-c"), None), &[PARENT.to_string(), "parent0002".to_string()], &TellAs::default(), &done, false);
        settle().await;
        let line = notify_line(FROM, &done, Some(NotifyLength::Whole));
        let said = ctx.recorded_json();
        assert_eq!(said.iter().map(|e| e["notify"].clone()).collect::<Vec<_>>(), vec![json!(PARENT), json!("parent0002")]);
        assert_eq!(said.iter().map(|e| e["text"].clone()).collect::<Vec<_>>(), vec![json!(line), json!(line)]);
        let started = ctx.started.borrow();
        assert_eq!(
            started.iter().map(|s| s.1.thread.clone().unwrap_or_default()).collect::<Vec<_>>(),
            vec![PARENT.to_string(), "parent0002".to_string()]
        );
        assert_eq!(started.iter().map(|s| s.1.prompt.clone()).collect::<Vec<_>>(), vec![line.clone(), line.clone()]);
        let log = ctx.store.logged();
        assert_eq!(log.len(), 4, "one line per target, each its own row, each written and then taken");
        for target in [PARENT, "parent0002"] {
            let put = log.iter().position(|l| *l == format!("put notify-owed/u-c:0:{target}")).expect("written");
            let delete = log.iter().position(|l| *l == format!("delete notify-owed/u-c:0:{target}")).expect("taken");
            assert!(put < delete, "{target}");
        }
    });
}

// TS: runtime-notify.test.ts:561
#[test]
fn a_target_whose_thread_is_gone_falls_back_to_the_person_once_with_me_already_named() {
    local(async {
        let (ctx, area) = owed_host();
        let done = result_of(json!({ "status": "completed", "text": "done" }));
        area.notify_end(&child_row(Some("cs-c"), None), &["gone0001".to_string(), "me".to_string()], &TellAs::default(), &done, false);
        settle().await;
        let said = ctx.recorded_json();
        assert_eq!(said.iter().map(|e| e["notify"].clone()).collect::<Vec<_>>(), vec![json!("me")], "told once however many fell away");
        assert_eq!(said[0]["text"], json!(notify_line(FROM, &done, Some(NotifyLength::Tail))));
        assert!(ctx.started.borrow().is_empty());
        assert!(ctx.store.logged().is_empty());
    });
}

// TS: runtime-notify.test.ts:600
#[test]
fn the_line_into_a_thread_carries_the_final_message_whole_and_the_persons_stays_its_last_line() {
    local(async {
        let (ctx, area) = owed_host();
        let (h, _tx) = handle("r-n", "u-n", SessionStartOutcome::Steered, None);
        ctx.starts.borrow_mut().push_back(Ok(h));
        let report = result_of(
            json!({ "status": "completed", "text": "Ran the gate.\n\nAll 12 tests green.\n", "durationMs": 492_000, "costUsd": 1.94 }),
        );
        area.notify_end(&child_row(Some("cs-c"), None), &[PARENT.to_string()], &TellAs::default(), &report, false);
        let mine = row(view(json!({ "id": "r-m", "workspaceId": "ws-c", "threadId": "mine0001", "claudeSessionId": "cs-m" })), "u-m");
        area.notify_end(&mine, &["me".to_string()], &TellAs::default(), &report, false);
        settle().await;
        let said = ctx.recorded_json();
        let whole = "thread child000 finished (completed, 8m 12s, $1.94): Ran the gate.\n\nAll 12 tests green.";
        assert_eq!((said[0]["notify"].clone(), said[0]["text"].clone()), (json!(PARENT), json!(whole)));
        assert_eq!(
            (said[1]["notify"].clone(), said[1]["text"].clone()),
            (json!("me"), json!(notify_line("mine0001", &report, Some(NotifyLength::Tail))))
        );
        assert_ne!(said[1]["text"].as_str().map(|t| t.contains("Ran the gate.")), Some(true), "the person's row is one line, the last");
        assert_eq!(ctx.started.borrow()[0].1.prompt, whole);
    });
}

// =====================================================================================================================
// steer-unread.test.ts: a steered message its agent never read
// =====================================================================================================================

// TS: steer-unread.test.ts:131
#[test]
fn an_unread_message_goes_back_opened_by_whoever_opened_it_and_as_whoever_steered_it() {
    let lead = json!({ "kind": "thread", "threadId": "lead", "workspaceId": "ws1", "rootThreadId": "lead" });
    for (steered, started_by, caller) in [
        (json!({ "prompt": "also add a test", "startedBy": "person" }), "person", None),
        (
            json!({ "prompt": "thread 1234abcd finished (completed): reply with the single word BANANA", "startedBy": "agent", "by": lead, "road": "relayed" }),
            "agent",
            Some(Caller::Scoped { origin: origin("relayed"), by: scope_of("lead", "ws1", "lead") }),
        ),
    ] {
        local(async move {
            let (ctx, area) = owed_host();
            let (h, _tx) = handle("r-n", "u-n", SessionStartOutcome::Started, None);
            ctx.starts.borrow_mut().push_back(Ok(h));
            let prompt = steered["prompt"].as_str().expect("a prompt").to_string();
            area.send_back(&unread_row(steered), &["st1".to_string()], false);
            settle().await;
            let started = ctx.started.borrow();
            assert_eq!(started.len(), 1);
            assert_eq!(started[0].1.prompt, prompt);
            assert_eq!(
                started[0].1.started_by,
                Some(serde_json::from_value(json!(started_by)).expect("an origin")),
                "opened by whoever opened it"
            );
            assert_eq!(started[0].2, caller, "under the caller that steered it");
            assert!(!started[0].1.wakes_lead, "a steered message is no child's report");
            assert_eq!(started[0].1.thread.as_deref(), Some(PARENT));
        });
    }
}

// TS: steer-unread.test.ts:117, 340
#[test]
fn an_unread_message_steered_from_a_paired_computer_is_told_to_the_person_and_starts_no_turn() {
    local(async {
        let (ctx, area) = owed_host();
        let warned = wsp_core::console::capture();
        area.send_back(
            &unread_row(json!({ "prompt": "also add a test", "startedBy": "person", "road": "paired" })),
            &["st1".to_string()],
            false,
        );
        settle().await;
        assert_eq!(
            warned.lines(),
            vec!["thread parent00 ended, but its line was registered from a paired computer, which starts no turn on thread parent00"]
        );
        assert!(ctx.started.borrow().is_empty());
        let told = told_person(&ctx.recorded_json());
        assert_eq!(told.len(), 1);
        assert_eq!((told[0]["threadId"].clone(), told[0]["text"].clone()), (json!(PARENT), json!(unread_line("also add a test"))));
        assert!(ctx.store.get_now("notify-owed", "u-t:unread:st1").is_none(), "the line fell and left the store");
    });
}

// TS: steer-unread.test.ts:152, 174
#[test]
fn an_unread_message_held_for_a_napping_workspace_goes_at_the_wake_with_its_images_after_a_restart() {
    local(async {
        let (ctx1, area1) = owed_host();
        set_phase(&ctx1, "ws1", "napping");
        let steered = json!({ "prompt": "\n  look at this  \nmore", "startedBy": "person", "requestId": "req-s", "attachments": [{ "mediaType": "image/png", "bytes": 3 }] });
        area1.send_back(&unread_row(steered), &["st1".to_string()], false);
        settle().await;
        assert!(ctx1.started.borrow().is_empty(), "held for the wake");
        let kept = ctx1.store.get_now("notify-owed", "u-t:unread:st1").expect("kept as a line");
        assert_eq!(kept["images"], json!({ "under": "req-s", "records": [{ "mediaType": "image/png", "bytes": 3 }] }));
        // The next host reads the store and holds the line again, since the workspace still naps.
        let (ctx2, area2) = owed_host();
        set_phase(&ctx2, "ws1", "napping");
        ctx2.store.seed("notify-owed", "u-t:unread:st1", kept.clone());
        ctx2.store.seed_blob("attachments", &format!("{PARENT}.req-s.0"), &[1, 2, 3]);
        area2.deliver_owed().await.expect("listed");
        settle().await;
        assert!(ctx2.started.borrow().is_empty());
        assert_eq!(area2.held_lines.borrow().get("ws1").map(|l| l.len()), Some(1));
        set_phase(&ctx2, "ws1", "running");
        let (h, _tx) = handle("r-n", "u-n", SessionStartOutcome::Started, None);
        ctx2.starts.borrow_mut().push_back(Ok(h));
        ctx2.bus.emit_json(woken("ws1"));
        settle().await;
        let started = ctx2.started.borrow();
        assert_eq!(started.len(), 1);
        assert_eq!(started[0].1.prompt, "\n  look at this  \nmore");
        assert_eq!(started[0].1.request_id.as_deref(), kept["requestId"].as_str());
        assert_eq!(
            support::json_of(started[0].1.attachments.as_ref().expect("its images")),
            json!([{ "mediaType": "image/png", "bytes": "AQID" }]),
            "read back from what the host kept of the steer"
        );
    });
}

// =====================================================================================================================
// steer-lines.test.ts: a child's finished lines into its lead
// =====================================================================================================================

// TS: steer-lines.test.ts:101, 115
#[test]
fn a_reply_over_background_work_and_the_answer_after_it_are_two_lines_and_only_the_open_one_skips_the_done_read() {
    local(async {
        let (ctx, area) = owed_host();
        let (sub, _) = entry(
            json!({ "id": "ws-c", "phase": "running", "sub": { "on": "hetzner", "repo": "r", "base": { "branch": "main", "sha": "abc" }, "branch": "b", "folder": "/f" } }),
        );
        ctx.add_entry(sub);
        set_phase(&ctx, "ws1", "napping");
        let reply =
            result_of(json!({ "status": "completed", "text": "Built the parser; 40 tests pass.\n\n1 background task is still running" }));
        let answer = result_of(json!({ "status": "completed", "text": "BANANA" }));
        area.notify_end(&child_row(Some("cs-c"), Some(0)), &[PARENT.to_string()], &TellAs::default(), &reply, true);
        area.notify_end(&child_row(Some("cs-c"), Some(1)), &[PARENT.to_string()], &TellAs::default(), &answer, false);
        settle().await;
        let first = ctx.store.get_now("notify-owed", &format!("u-c:0:{PARENT}")).expect("the reply's line");
        let second = ctx.store.get_now("notify-owed", &format!("u-c:1:{PARENT}")).expect("the answer's line, kept apart");
        assert_eq!(first["text"], json!(notify_line(FROM, &reply, Some(NotifyLength::Whole))));
        assert_eq!(second["text"], json!(notify_line(FROM, &answer, Some(NotifyLength::Whole))));
        assert_eq!(first.get("awaitsResult"), None, "a reply given over work still running has no done behind it yet");
        assert_eq!(second["awaitsResult"], json!("ws-c"), "a sub-thread's own end reads its done off the machine");
        assert!(!ctx.called().iter().any(|c| c.starts_with("subLine")), "held for the wake, so nothing is read yet");
        let held: Vec<String> =
            area.held_lines.borrow().get("ws1").map(|l| l.iter().map(|x| x.borrow().id.clone()).collect()).unwrap_or_default();
        assert_eq!(held, vec![format!("u-c:0:{PARENT}"), format!("u-c:1:{PARENT}")], "held in the order told");
    });
}

// TS: thread-cap.test.ts:78
#[test]
fn a_leads_child_runs_in_the_leads_slot_and_a_thread_of_another_tree_waits_for_both() {
    let (ctx, area, record) = cap_host(1, 1);
    start_row(&ctx, "n1", "tn");
    area.cap_hold(&record, "n1", Some("u0"));
    assert_eq!(
        area.cap_full(&record, "n1"),
        None,
        "the child runs in the lead's slot, so the lead waiting on it is never stuck behind itself"
    );
    area.cap_left("n1");
    assert_eq!(borrowed_of(&area, "n1").as_deref(), Some("u0"));
    start_row(&ctx, "n2", "to");
    area.cap_hold(&record, "n2", None);
    assert_eq!(area.cap_full(&record, "n2"), Some(cap_wait_of(2, 1)), "the row counts both agents, so a thread of another tree waits");
    turn_ends(&ctx, "u0");
    assert_eq!(borrowed_of(&area, "n1"), None, "the child holds a slot of its own once its lender ended");
    assert_eq!(area.cap_full(&record, "n2"), Some(cap_wait_of(1, 1)));
    turn_ends(&ctx, "n1");
    assert_eq!(area.cap_full(&record, "n2"), None);
}
