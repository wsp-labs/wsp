// SPDX-License-Identifier: AGPL-3.0-only
//! Part 2 of `agents_lists.rs`: the same world, the tests after it.
#![allow(dead_code, unused_imports)]

#[path = "support_b/mod.rs"]
mod support;
include!("agents_lists/world.rs");

// TS: session-title.test.ts:213
#[test]
fn refresh_title_fails_one_rows_title_never_the_call_when_the_adapter_refuses_the_id() {
    local(async {
        let ctx = FakeAgentsCtx::new();
        let adapter = FakeAdapter::new().titled(Ability::failing("the session id is not a plain slug")).arc();
        running(&ctx, "claude", &adapter);
        seed(&ctx, thread_rows());
        let area = ctx.area();
        let v = ctx.sessions.borrow().get("r1").expect("a row").borrow().view.clone();
        let warned = console::capture();
        area.refresh_title(&v, false).await;
        assert!(row_json(&ctx, "r1").get("harnessTitle").is_none());
        assert_eq!(warned.lines(), ["no title for session S1 on ws1: the session id is not a plain slug"]);
    });
}

// TS: session-title.test.ts:362, session-title.test.ts:320, session-title.test.ts:341
#[test]
fn refresh_title_warns_once_until_a_read_answers() {
    local(async {
        let ctx = FakeAgentsCtx::new();
        let slow = || Err(other("timed out after 15000 ms"));
        let adapter = FakeAdapter::new()
            .titled(Ability::sequence(vec![slow(), slow(), slow(), slow(), Ok(Some("Building the server".to_string())), slow()]))
            .arc();
        running(&ctx, "claude", &adapter);
        seed(&ctx, thread_rows());
        let area = ctx.area();
        let v = ctx.sessions.borrow().get("r1").expect("a row").borrow().view.clone();
        let warned = console::capture();
        let line = "no title for session S1 on ws1: timed out after 15000 ms".to_string();
        for _ in 0..4 {
            area.refresh_title(&v, false).await;
            ctx.clock.advance(SESSION_TITLE_TTL_MS as i64 + 1);
        }
        assert_eq!(adapter.called().len(), 4, "a failed read is a read: the window holds");
        assert_eq!(warned.lines(), std::slice::from_ref(&line), "said once while it keeps failing");
        // A read that answers clears the mark, so the next failure is said again.
        area.refresh_title(&v, false).await;
        assert_eq!(row_json(&ctx, "r1")["harnessTitle"], "Building the server");
        ctx.clock.advance(SESSION_TITLE_TTL_MS as i64 + 1);
        area.refresh_title(&v, false).await;
        assert_eq!(warned.lines(), [line.clone(), line]);
        assert_eq!(adapter.called().len(), 6);
    });
}

// TS: session-title.test.ts:182, session-title.test.ts:866
#[test]
fn refresh_title_does_nothing_on_an_unreached_machine() {
    local(async {
        let ctx = FakeAgentsCtx::new();
        let adapter = FakeAdapter::new().titled(titled("Building the server")).arc();
        running(&ctx, "claude", &adapter);
        seed(&ctx, thread_rows());
        let area = ctx.area();
        let v = ctx.sessions.borrow().get("r1").expect("a row").borrow().view.clone();

        ctx.unreached.borrow_mut().insert("ws1".to_string(), "the machine did not answer".to_string());
        area.refresh_title(&v, true).await;
        ctx.unreached.borrow_mut().clear();

        // A workspace that is not running keeps the title it last read and is asked nothing.
        let (napping, machine) = entry(json!({ "phase": "napping" }));
        ctx.add_entry(napping);
        area.refresh_title(&v, true).await;
        assert!(machine.commands().is_empty());

        // A workspace that is not live, a row with no agent session, a harness with no adapter.
        let mut elsewhere = v.clone();
        elsewhere.workspace_id = "ws-gone".to_string();
        area.refresh_title(&elsewhere, true).await;
        let mut sessionless = v.clone();
        sessionless.claude_session_id = None;
        area.refresh_title(&sessionless, true).await;
        let mut unknown = v.clone();
        unknown.harness = "aider".into();
        area.refresh_title(&unknown, true).await;

        assert!(adapter.called().is_empty());
        assert!(row_json(&ctx, "r1").get("harnessTitle").is_none());
        assert!(effects(&ctx).is_empty());
    });
}

// TS: session-title.test.ts:458, session-title.test.ts:617
#[test]
fn refresh_title_keeps_a_persons_name_over_the_seed() {
    local(async {
        let ctx = FakeAgentsCtx::new();
        // The store answers the opening words, which codex writes there the moment a thread starts: the seed again.
        let adapter = FakeAdapter::new()
            .titled(titled(OPENING))
            .renaming(Ability::answering(SessionRenameWrite::Written).execing("wsp-title-write S1"))
            .arc();
        let (_, machine) = running(&ctx, "claude", &adapter);
        seed(
            &ctx,
            vec![
                json!({ "id": "r1", "threadId": "t1", "claudeSessionId": "S1", "prompt": OPENING, "startedAt": 10, "harnessTitle": "Ticket 411 review", "titleSource": "person" }),
            ],
        );
        let area = ctx.area();
        let v = ctx.sessions.borrow().get("r1").expect("a row").borrow().view.clone();

        area.refresh_title(&v, false).await;
        settle().await;
        let r = row_json(&ctx, "r1");
        assert_eq!((r["harnessTitle"].clone(), r["titleSource"].clone()), (json!("Ticket 411 review"), json!("person")));
        assert_eq!(effects(&ctx), ["persist ws1"], "nothing moved, so no row is pushed and no name is written");
        assert!(adapter.called().iter().all(|c| !c.starts_with("renameSession")));

        // Forced, at a turn's end: the row's own name is written into the store again, since the index row is there now.
        ctx.clock.advance(SESSION_TITLE_TTL_MS as i64 + 1);
        area.refresh_title(&v, true).await;
        settle().await;
        assert_eq!(
            adapter.called().iter().filter(|c| c.starts_with("renameSession")).cloned().collect::<Vec<_>>(),
            ["renameSession S1 Ticket 411 review"]
        );
        assert_eq!(runs(&machine, "wsp-title-write S1"), [Some(15_000)]);
        assert!(ctx.called().contains(&"writeSession S1".to_string()));
        assert_eq!(row_json(&ctx, "r1")["harnessTitle"], "Ticket 411 review");
    });
}

// TS: session-title.test.ts:475
#[test]
fn refresh_title_reads_the_opening_words_as_the_seed_whole_or_by_its_head() {
    local(async {
        for stored in [OPENING, "make a server"] {
            let ctx = FakeAgentsCtx::new();
            let adapter = FakeAdapter::new().titled(titled(stored)).arc();
            running(&ctx, "claude", &adapter);
            seed(&ctx, vec![json!({ "id": "r1", "threadId": "t1", "claudeSessionId": "S1", "prompt": OPENING, "startedAt": 10 })]);
            let area = ctx.area();
            let v = ctx.sessions.borrow().get("r1").expect("a row").borrow().view.clone();
            area.refresh_title(&v, false).await;
            let r = row_json(&ctx, "r1");
            assert_eq!((r["harnessTitle"].clone(), r["titleSource"].clone()), (json!(stored), json!("seed")), "{stored}");
            assert_eq!(area.thread_source("t1"), TitleSource::Seed, "{stored}: the thread is still to be asked for a name");
        }
    });
}

// TS: session-title.test.ts:490
#[test]
fn refresh_title_reads_a_title_that_is_the_whole_of_a_longer_opening_as_the_seed() {
    local(async {
        let ctx = FakeAgentsCtx::new();
        let brief = format!("make a server, and its tests. {}", "Read the ticket and every comment on it before the code. ".repeat(40));
        // The row keeps only the head of the opening: 600 units.
        let kept: String = brief.chars().take(600).collect();
        let adapter = FakeAdapter::new().titled(titled(&brief)).arc();
        running(&ctx, "claude", &adapter);
        seed(&ctx, vec![json!({ "id": "r1", "threadId": "t1", "claudeSessionId": "S1", "prompt": kept, "startedAt": 10 })]);
        let v = ctx.sessions.borrow().get("r1").expect("a row").borrow().view.clone();
        ctx.area().refresh_title(&v, false).await;
        assert_eq!(row_json(&ctx, "r1")["titleSource"], "seed");
    });
}

// TS: session-title.test.ts:504
#[test]
fn refresh_title_keeps_the_name_it_made_when_the_store_still_answers_the_opening_words() {
    local(async {
        let ctx = FakeAgentsCtx::new();
        let adapter = FakeAdapter::new().titled(titled(OPENING)).arc();
        running(&ctx, "claude", &adapter);
        seed(
            &ctx,
            vec![
                json!({ "id": "r1", "threadId": "t1", "claudeSessionId": "S1", "prompt": OPENING, "startedAt": 10, "harnessTitle": "Seed thread titles here", "titleSource": "auto" }),
            ],
        );
        let area = ctx.area();
        let v = ctx.sessions.borrow().get("r1").expect("a row").borrow().view.clone();
        area.refresh_title(&v, false).await;
        ctx.clock.advance(SESSION_TITLE_TTL_MS as i64 + 1);
        area.refresh_title(&v, false).await;
        let r = row_json(&ctx, "r1");
        assert_eq!((r["harnessTitle"].clone(), r["titleSource"].clone()), (json!("Seed thread titles here"), json!("auto")));
        assert!(!effects(&ctx).iter().any(|e| e.starts_with("pushHead")));
    });
}

// TS: session-title.test.ts:645, session-title.test.ts:662
#[test]
fn refresh_title_keeps_a_start_named_thread_whose_store_titles_it_with_the_whole_brief() {
    local(async {
        let brief = "You are the cold reviewer for wsp-map #1671: the desktop app\nupdating itself in place.\n\nRead the diff first.";
        let with_files = format!("{brief}\n\nAttached files:\n- /root/app/.wsp-files/t1/req_a/notes.txt");
        for stored in [brief.to_string(), with_files] {
            let ctx = FakeAgentsCtx::new();
            let adapter = FakeAdapter::new().titled(titled(&stored)).arc();
            running(&ctx, "claude", &adapter);
            seed(
                &ctx,
                vec![
                    json!({ "id": "r1", "threadId": "t1", "claudeSessionId": "S1", "prompt": brief, "startedAt": 10, "harnessTitle": "Review 1671 self update", "titleSource": "person" }),
                ],
            );
            let v = ctx.sessions.borrow().get("r1").expect("a row").borrow().view.clone();
            ctx.area().refresh_title(&v, false).await;
            let r = row_json(&ctx, "r1");
            assert_eq!(
                (r["harnessTitle"].clone(), r["titleSource"].clone()),
                (json!("Review 1671 self update"), json!("person")),
                "{stored}"
            );
        }
    });
}

// TS: sub-resume.test.ts:446, sub-resume.test.ts:454
#[test]
fn refresh_title_keeps_a_name_when_the_new_machines_store_titles_it_with_the_threads_opening_or_the_lost_session_words() {
    local(async {
        let preamble = format!("You are a sub-thread of wsp. Remember the word lantern. {OPENING}");
        let lost = format!("{LOST_HEAD} Run git log --oneline -1.");
        for stored in [preamble.clone(), lost] {
            let ctx = FakeAgentsCtx::new();
            let adapter = FakeAdapter::new().titled(titled(&stored)).arc();
            running(&ctx, "claude", &adapter);
            seed(
                &ctx,
                vec![
                    json!({ "id": "r1", "threadId": "t1", "claudeSessionId": "S0", "prompt": preamble, "startedAt": 10, "harnessTitle": "Fix the upload test", "titleSource": "person" }),
                    json!({ "id": "r2", "threadId": "t1", "claudeSessionId": "S1", "prompt": "Run git log --oneline -1.", "startedAt": 20, "harnessTitle": "Fix the upload test", "titleSource": "person" }),
                ],
            );
            let v = ctx.sessions.borrow().get("r2").expect("a row").borrow().view.clone();
            ctx.area().refresh_title(&v, false).await;
            let r = row_json(&ctx, "r2");
            assert_eq!((r["harnessTitle"].clone(), r["titleSource"].clone()), (json!("Fix the upload test"), json!("person")), "{stored}");
        }
    });
}

// ---- L6: where a title came from -------------------------------------------------------------------------------------

#[test]
fn thread_source_is_person_over_auto_over_seed() {
    local(async {
        let ctx = FakeAgentsCtx::new();
        let area = ctx.area();
        seed(&ctx, vec![json!({ "id": "r1", "threadId": "t1" }), json!({ "id": "r2", "threadId": "t1", "titleSource": "seed" })]);
        assert_eq!(area.thread_source("t1"), TitleSource::Seed);
        assert_eq!(area.thread_source("t-none"), TitleSource::Seed);
        seed(&ctx, vec![json!({ "id": "r1", "threadId": "t1" }), json!({ "id": "r2", "threadId": "t1", "titleSource": "auto" })]);
        assert_eq!(area.thread_source("t1"), TitleSource::Auto);
        seed(
            &ctx,
            vec![
                json!({ "id": "r1", "threadId": "t1", "titleSource": "auto" }),
                json!({ "id": "r2", "threadId": "t1", "titleSource": "person" }),
                json!({ "id": "r3", "threadId": "t1" }),
            ],
        );
        assert_eq!(area.thread_source("t1"), TitleSource::Person, "a person's name on any turn is the thread's");
        let bare = view(json!({ "id": "x" }));
        assert_eq!(source_of(&bare), TitleSource::Seed, "a row with no provenance reads as seed");
        assert_eq!(source_of(&view(json!({ "id": "x", "titleSource": "auto" }))), TitleSource::Auto);
    });
}

#[test]
fn rows_on_lists_the_threads_rows_in_map_order_and_the_opening_is_the_oldest_rows_prompt() {
    local(async {
        let ctx = FakeAgentsCtx::new();
        let area = ctx.area();
        seed(
            &ctx,
            vec![
                json!({ "id": "r1", "threadId": "t1", "prompt": "second", "startedAt": 20 }),
                json!({ "id": "r2", "threadId": "t2", "prompt": "elsewhere", "startedAt": 1 }),
                json!({ "id": "r3", "threadId": "t1", "prompt": "first", "startedAt": 10 }),
                json!({ "id": "r4", "threadId": "t1", "prompt": "tied", "startedAt": 10 }),
            ],
        );
        assert_eq!(area.rows_on("t1").iter().map(|v| v.id.clone()).collect::<Vec<_>>(), ["r1", "r3", "r4"]);
        assert_eq!(area.opening_of("t1"), Some("first".to_string()), "the first on a tie");
        assert_eq!(area.opening_of("t9"), None);
        // An absent start reads as zero.
        seed(
            &ctx,
            vec![
                json!({ "id": "r1", "threadId": "t1", "prompt": "later", "startedAt": 5 }),
                json!({ "id": "r2", "threadId": "t1", "prompt": "no start" }),
            ],
        );
        assert_eq!(area.opening_of("t1"), Some("no start".to_string()));
    });
}

#[test]
fn carried_title_is_the_newest_titled_row() {
    local(async {
        let ctx = FakeAgentsCtx::new();
        let area = ctx.area();
        seed(
            &ctx,
            vec![
                json!({ "id": "r1", "threadId": "t1", "startedAt": 10, "harnessTitle": "Old", "titleSource": "person" }),
                json!({ "id": "r2", "threadId": "t1", "startedAt": 30, "harnessTitle": "New", "titleSource": "auto" }),
                json!({ "id": "r3", "threadId": "t1", "startedAt": 50 }),
                json!({ "id": "r4", "threadId": "t2", "startedAt": 90, "harnessTitle": "Other thread" }),
            ],
        );
        assert_eq!(
            area.carried_title("t1"),
            CarriedTitle { harness_title: Some("New".to_string()), title_source: Some(TitleSource::Auto) }
        );
        // A row with a title and no provenance carries seed.
        seed(&ctx, vec![json!({ "id": "r1", "threadId": "t1", "startedAt": 10, "harnessTitle": "Written before provenance" })]);
        assert_eq!(
            area.carried_title("t1"),
            CarriedTitle { harness_title: Some("Written before provenance".to_string()), title_source: Some(TitleSource::Seed) }
        );
        seed(&ctx, vec![json!({ "id": "r1", "threadId": "t1", "startedAt": 10 })]);
        assert_eq!(area.carried_title("t1"), CarriedTitle::default(), "no titled row is empty");
    });
}

// ---- L7: naming a session in the harness -----------------------------------------------------------------------------

// TS: session-title.test.ts:689, session-title.test.ts:748, session-title.test.ts:801
#[test]
fn name_in_harness_writes_through_write_session_and_warns_on_refusal() {
    local(async {
        let ctx = FakeAgentsCtx::new();
        let adapter = FakeAdapter::new()
            .renaming(
                Ability::sequence(vec![
                    Ok(SessionRenameWrite::Written),
                    Ok(SessionRenameWrite::NoSession),
                    Ok(SessionRenameWrite::Failed { error: "database is locked".to_string() }),
                    Err(other("the machine hung up")),
                ])
                .execing("wsp-title-write S1"),
            )
            .arc();
        let (_, machine) = running(&ctx, "claude", &adapter);
        let area = ctx.area();
        // The harness's own session is named, whatever the runtime calls the row.
        let v = view(json!({ "id": "row-not-the-session", "claudeSessionId": "S1", "threadId": "t1" }));
        let warned = console::capture();

        area.name_in_harness(&v, "the name he typed in wsp").await;
        assert_eq!(adapter.called(), ["renameSession S1 the name he typed in wsp"]);
        assert_eq!(runs(&machine, "wsp-title-write S1"), [Some(15_000)]);
        assert_eq!(ctx.called_with("writeSession"), ["writeSession S1"]);
        assert!(warned.lines().is_empty());

        // The store took nothing: no line, the name stands.
        area.name_in_harness(&v, "second").await;
        assert!(warned.lines().is_empty());
        // The store refused: one line with its own words. A write that failed on the way: the same line, the message.
        area.name_in_harness(&v, "third").await;
        area.name_in_harness(&v, "fourth").await;
        assert_eq!(
            warned.lines(),
            [
                "the harness did not take the name for session S1 on ws1: database is locked",
                "the harness did not take the name for session S1 on ws1: the machine hung up",
            ]
        );
    });
}

// TS: session-title.test.ts:726, session-title.test.ts:760, session-title.test.ts:790
#[test]
fn name_in_harness_does_nothing_where_nothing_can_be_written() {
    local(async {
        let ctx = FakeAgentsCtx::new();
        let plain = FakeAdapter::new().arc();
        let (_, machine) = running(&ctx, "claude", &plain);
        let area = ctx.area();
        // An adapter that renames nothing, and its machine is asked nothing.
        area.name_in_harness(&view(json!({ "claudeSessionId": "S1" })), "a name").await;
        assert!(machine.commands().is_empty());
        assert!(ctx.called_with("writeSession").is_empty());

        let ctx = FakeAgentsCtx::new();
        let adapter = FakeAdapter::new().renaming(Ability::answering(SessionRenameWrite::Written)).arc();
        running(&ctx, "claude", &adapter);
        let area = ctx.area();
        // A row whose harness never named a session; a harness with no adapter; a workspace that is not live.
        area.name_in_harness(&view(json!({})), "a name").await;
        area.name_in_harness(&view(json!({ "claudeSessionId": "S1", "harness": "aider" })), "a name").await;
        area.name_in_harness(&view(json!({ "claudeSessionId": "S1", "workspaceId": "ws-gone" })), "a name").await;
        // A workspace that is not running keeps the name only on its rows.
        let (napping, _) = entry(json!({ "id": "ws2", "phase": "napping" }));
        ctx.add_entry(napping);
        area.name_in_harness(&view(json!({ "claudeSessionId": "S1", "workspaceId": "ws2" })), "a name").await;
        assert!(adapter.called().is_empty());
        assert!(ctx.called_with("writeSession").is_empty());
    });
}

// TS: session-title.test.ts:398, session-title.test.ts:449, session-title.test.ts:580
#[test]
fn make_title_asks_once_per_thread_and_names_every_row() {
    local(async {
        let ctx = FakeAgentsCtx::new();
        let adapter = FakeAdapter::new()
            .titling(maker("Seed thread titles here").execing("wsp-title-make"))
            .renaming(Ability::answering(SessionRenameWrite::Written))
            .arc();
        let (_, machine) = running(&ctx, "claude", &adapter);
        seed(
            &ctx,
            vec![
                json!({ "id": "r1", "threadId": "t1", "claudeSessionId": "S1", "prompt": OPENING, "startedAt": 10 }),
                json!({ "id": "r2", "threadId": "t1", "claudeSessionId": "S1", "prompt": "and now the tests", "startedAt": 20 }),
                json!({ "id": "r3", "threadId": "t2", "claudeSessionId": "S9", "prompt": "other", "startedAt": 30 }),
            ],
        );
        let area = ctx.area();
        let v = ctx.sessions.borrow().get("r1").expect("a row").borrow().view.clone();

        area.make_title(&v).await.expect("asked");
        // From the opening turn alone and on the cheapest model its catalog lists.
        let turns = adapter.title_turns.lock().expect("turns").clone();
        assert_eq!(turns.len(), 1);
        assert_eq!(
            (turns[0].opening.as_str(), turns[0].reply.as_deref(), turns[0].model.as_deref()),
            (OPENING, None, Some("claude-haiku-5-5"))
        );
        assert_eq!(runs(&machine, "wsp-title-make"), [Some(25_000)]);
        for id in ["r1", "r2"] {
            let r = row_json(&ctx, id);
            assert_eq!((r["harnessTitle"].clone(), r["titleSource"].clone()), (json!("Seed thread titles here"), json!("auto")), "{id}");
        }
        assert!(row_json(&ctx, "r3").get("harnessTitle").is_none());
        // Kept, pushed, then written into the harness's own store.
        assert_eq!(effects(&ctx), ["persist ws1", "pushHead t1", "writeSession S1"]);
        assert_eq!(
            adapter.called().iter().filter(|c| c.starts_with("renameSession")).cloned().collect::<Vec<_>>(),
            ["renameSession S1 Seed thread titles here"]
        );

        // Once per thread: the second turn of it asks nothing.
        let second = ctx.sessions.borrow().get("r2").expect("a row").borrow().view.clone();
        area.make_title(&second).await.expect("nothing to ask");
        area.make_title(&v).await.expect("nothing to ask");
        assert_eq!(adapter.title_turns.lock().expect("turns").len(), 1);
    });
}

// TS: session-title.test.ts:522
#[test]
fn make_title_leaves_a_thread_a_person_named_meanwhile() {
    local(async {
        let ctx = FakeAgentsCtx::new();
        let gate = Gate::new();
        let adapter = FakeAdapter::new().titling(maker("Seed thread titles here").gated(&gate)).arc();
        running(&ctx, "claude", &adapter);
        seed(&ctx, vec![json!({ "id": "r1", "threadId": "t1", "claudeSessionId": "S1", "prompt": OPENING, "startedAt": 10 })]);
        let area = ctx.area();
        let v = ctx.sessions.borrow().get("r1").expect("a row").borrow().view.clone();
        let asking = tokio::task::spawn_local({
            let area = area.clone();
            async move { area.make_title(&v).await }
        });
        settle().await;
        assert_eq!(adapter.called(), ["titleFor"], "the question is out");
        // The person renames the thread inside the harness while it thinks.
        {
            let rows = ctx.sessions.borrow();
            let mut r = rows.get("r1").expect("a row").borrow_mut();
            r.view.harness_title = Some("the sidebar's own name".to_string());
            r.view.title_source = Some(TitleSource::Person);
        }
        gate.open();
        asking.await.expect("ran").expect("not a failure");
        let r = row_json(&ctx, "r1");
        assert_eq!((r["harnessTitle"].clone(), r["titleSource"].clone()), (json!("the sidebar's own name"), json!("person")));
        assert!(effects(&ctx).is_empty(), "nothing is kept, pushed or written");
    });
}

// TS: session-title.test.ts:543
#[test]
fn make_title_keeps_the_opening_words_when_the_harness_names_nothing_and_asks_no_more() {
    local(async {
        let ctx = FakeAgentsCtx::new();
        let adapter = FakeAdapter::new().titling(Ability::answering(None)).arc();
        running(&ctx, "claude", &adapter);
        seed(
            &ctx,
            vec![json!({ "id": "r1", "threadId": "thread-0001-abcdef", "claudeSessionId": "S1", "prompt": OPENING, "startedAt": 10 })],
        );
        let area = ctx.area();
        let v = ctx.sessions.borrow().get("r1").expect("a row").borrow().view.clone();
        let warned = console::capture();
        area.make_title(&v).await.expect("not a failure");
        assert_eq!(warned.lines(), ["thread thread-0 on ws1 keeps its opening words: the harness answered with no title"]);
        assert!(row_json(&ctx, "r1").get("harnessTitle").is_none());
        area.make_title(&v).await.expect("not a failure");
        assert_eq!(adapter.called(), ["titleFor"], "nothing retries in a loop");
    });
}

// TS: title-after-close.test.ts:136
#[test]
fn make_title_asks_nothing_once_the_host_is_closing() {
    local(async {
        let ctx = FakeAgentsCtx::new();
        let adapter = FakeAdapter::new().titling(maker("Seed thread titles here")).arc();
        running(&ctx, "claude", &adapter);
        seed(&ctx, vec![json!({ "id": "r1", "threadId": "t1", "claudeSessionId": "S1", "prompt": OPENING, "startedAt": 10 })]);
        let area = ctx.area();
        let v = ctx.sessions.borrow().get("r1").expect("a row").borrow().view.clone();
        ctx.closing.set(true);
        area.make_title(&v).await.expect("not a failure");
        assert!(adapter.called().is_empty());
        assert!(row_json(&ctx, "r1").get("harnessTitle").is_none());
    });
}

#[test]
fn make_title_does_nothing_for_a_row_that_is_not_to_be_asked() {
    local(async {
        let ctx = FakeAgentsCtx::new();
        let adapter = FakeAdapter::new().titling(maker("Seed thread titles here")).arc();
        running(&ctx, "claude", &adapter);
        let area = ctx.area();
        seed(
            &ctx,
            vec![
                json!({ "id": "r1", "threadId": "t1", "claudeSessionId": "S1", "prompt": OPENING }),
                json!({ "id": "r5", "threadId": "t5", "claudeSessionId": "S5", "prompt": OPENING, "titleSource": "person", "harnessTitle": "Named" }),
            ],
        );
        let get = |id: &str| ctx.sessions.borrow().get(id).expect("a row").borrow().view.clone();
        // No thread, no prompt, a thread a person named, a harness with no adapter, a workspace not live.
        let mut no_thread = get("r1");
        no_thread.thread_id = None;
        area.make_title(&no_thread).await.expect("nothing");
        let mut no_prompt = get("r1");
        no_prompt.prompt = None;
        area.make_title(&no_prompt).await.expect("nothing");
        area.make_title(&get("r5")).await.expect("nothing");
        let mut unknown = get("r1");
        unknown.harness = "aider".into();
        area.make_title(&unknown).await.expect("nothing");
        let mut gone = get("r1");
        gone.workspace_id = "ws-gone".into();
        area.make_title(&gone).await.expect("nothing");
        assert!(adapter.called().is_empty());
        // An adapter that cannot be asked at all is never marked asked, and asks nothing.
        let ctx = FakeAgentsCtx::new();
        let silent_adapter = FakeAdapter::new().arc();
        running(&ctx, "claude", &silent_adapter);
        seed(&ctx, vec![json!({ "id": "r1", "threadId": "t1", "claudeSessionId": "S1", "prompt": OPENING })]);
        let v = ctx.sessions.borrow().get("r1").expect("a row").borrow().view.clone();
        ctx.area().make_title(&v).await.expect("nothing");
        assert!(silent_adapter.called().is_empty());
    });
}

// ---- L9: which rows a refresh asks about ----------------------------------------------------------------------------

// TS: session-title.test.ts:235, session-title.test.ts:260
#[test]
fn title_rows_asks_the_newest_row_of_each_session_up_to_twenty() {
    let mut rows: Vec<_> = (0..22)
        .map(|i| view(json!({ "id": format!("row-{i:02}"), "claudeSessionId": format!("s-{i:02}"), "startedAt": 1000 + i * 10 })))
        .collect();
    // An older turn of the newest session, a turn that never named a session, and a tie on the oldest.
    rows.push(view(json!({ "id": "older-of-21", "claudeSessionId": "s-21", "startedAt": 5 })));
    rows.push(view(json!({ "id": "no-session", "startedAt": 99_999 })));
    let picked = title_rows(&rows);
    let ids: Vec<String> = picked.iter().map(|v| v.id.clone()).collect();
    let expected: Vec<String> = (2..22).rev().map(|i| format!("row-{i:02}")).collect();
    assert_eq!(ids, expected, "newest first, one row per session, rows with no session left out, at most 20");
    assert_eq!(picked.len(), SESSION_TITLE_REFRESH_MAX);

    // Two rows of one session at one start: the first in the list stands.
    let tied = vec![
        view(json!({ "id": "first", "claudeSessionId": "s", "startedAt": 7 })),
        view(json!({ "id": "second", "claudeSessionId": "s", "startedAt": 7 })),
    ];
    assert_eq!(title_rows(&tied).iter().map(|v| v.id.clone()).collect::<Vec<_>>(), ["first"]);
    // An absent start reads as zero.
    let zero = vec![
        view(json!({ "id": "unstamped", "claudeSessionId": "a" })),
        view(json!({ "id": "stamped", "claudeSessionId": "b", "startedAt": 3 })),
    ];
    assert_eq!(title_rows(&zero).iter().map(|v| v.id.clone()).collect::<Vec<_>>(), ["stamped", "unstamped"]);
}

// ---- L10: the lists a picker offers ----------------------------------------------------------------------------------

// TS: runtime-history.test.ts:429, runtime-history.test.ts:725, thread-access.test.ts:111, thread-defaults.test.ts:180
#[test]
fn agent_lists_answers_the_table_off_a_paused_machine() {
    local(async {
        let ctx = FakeAgentsCtx::new();
        // Registered codex first: the lists come in the table's order whatever the adapters' own.
        let codex = FakeAdapter::new().probing(answered("0.9.0")).arc();
        let claude = FakeAdapter::new().probing(answered("2.1.257")).arc();
        ctx.add_adapter("codex", plain_factory(codex.clone()));
        ctx.add_adapter("claude", plain_factory(claude.clone()));
        let area = ctx.area();

        // No entry named: wsp's table, one row per harness with an adapter; the rest cannot run a turn and are not listed.
        let listed: Vec<Value> = area.agent_lists(None).await.iter().map(json_of).collect();
        assert_eq!(listed.iter().map(|c| c["harness"].as_str().expect("a harness").to_string()).collect::<Vec<_>>(), ["claude", "codex"]);
        assert!(listed.iter().all(|c| c["source"] == "table"));
        assert!(!listed[0]["efforts"].as_array().expect("efforts").is_empty());

        // A workspace whose machine is not up answers the table too, the access its next thread runs at included.
        let (napping, machine) = entry(json!({ "phase": "napping" }));
        let paused: Vec<Value> = area.agent_lists(Some(&napping)).await.iter().map(json_of).collect();
        assert_eq!(paused, listed);
        let modes = paused[0]["permissionModes"].as_array().expect("modes").clone();
        assert_eq!(modes.iter().find(|m| m["isDefault"] == true).map(|m| m["value"].clone()), Some(json!("bypassPermissions")));
        assert!(machine.commands().is_empty(), "a napping workspace's binary is left alone");
        assert!(ctx.called().is_empty());
        assert!(claude.called().is_empty() && codex.called().is_empty());

        // An agent turned off on the computer the workspace stands on leaves the lists.
        ctx.turn_off("here", "claude");
        let (off, _) = entry(json!({ "phase": "napping" }));
        let without: Vec<Value> = area.agent_lists(Some(&off)).await.iter().map(json_of).collect();
        assert_eq!(without.iter().map(|c| c["harness"].as_str().expect("a harness").to_string()).collect::<Vec<_>>(), ["codex"]);
    });
}

#[test]
fn agent_lists_asks_each_agent_of_a_running_machine_at_once_in_table_order() {
    local(async {
        let ctx = FakeAgentsCtx::new();
        let codex = FakeAdapter::new().probing(answered("0.9.0")).arc();
        let claude = FakeAdapter::new().probing(answered("2.1.257")).arc();
        ctx.add_adapter("codex", plain_factory(codex.clone()));
        ctx.add_adapter("claude", plain_factory(claude.clone()));
        let (e, _) = entry(json!({}));
        let listed: Vec<Value> = ctx.area().agent_lists(Some(&e)).await.iter().map(json_of).collect();
        assert_eq!(listed.iter().map(|c| c["harness"].as_str().expect("a harness").to_string()).collect::<Vec<_>>(), ["claude", "codex"]);
        assert!(listed.iter().all(|c| c["source"] == "harness"));
        assert_eq!((listed[0]["version"].clone(), listed[1]["version"].clone()), (json!("2.1.257"), json!("0.9.0")));
        assert_eq!(ctx.called_with("knowBroker"), ["knowBroker ws1 claude", "knowBroker ws1 codex"]);
        assert_eq!(claude.called(), ["probeCatalog"]);
        assert_eq!(codex.called(), ["probeCatalog"]);
    });
}

// TS: places-agents.test.ts:190, thread-defaults.test.ts:288
#[test]
fn agent_lists_answers_a_refused_config_folder_as_its_refusal() {
    local(async {
        let root = std::fs::canonicalize(tempfile::tempdir().expect("a folder").keep()).expect("a real path");
        let home = root.join("home");
        let outside = root.join("outside");
        std::fs::create_dir_all(&home).expect("home");
        std::fs::create_dir_all(&outside).expect("outside");
        let kept = outside.join("claude-wsp");
        let kept = kept.to_str().expect("utf8").to_string();

        let ctx = FakeAgentsCtx::new();
        *ctx.local_home_dir.borrow_mut() = Some(home.to_str().expect("utf8").to_string());
        ctx.keep_config_dir("here", "claude", &kept);
        let claude = FakeAdapter::new().arc();
        let codex = FakeAdapter::new().arc();
        ctx.add_adapter("claude", plain_factory(claude));
        ctx.add_adapter("codex", plain_factory(codex));
        let (e, _) = entry(json!({}));
        let area = ctx.area();
        let listed: Vec<Value> = area.agent_lists(Some(&e)).await.iter().map(json_of).collect();

        let home = home.to_str().expect("utf8");
        let words = format!(
            "Claude Code does not start with its config folder {kept}: Claude Code's config folder has to be under the home folder {home}, and {kept} is not. Set another with wsp agents setup claude --config, or put its own back with --reset config."
        );
        assert_eq!(listed[0]["harness"], "claude");
        assert_eq!(listed[0]["refusal"], json!(words));
        assert_eq!(listed[1]["harness"], "codex");
        assert!(listed[1].get("refusal").is_none());
        // The refused agent is not asked for its broker; the word moved once.
        assert_eq!(ctx.called_with("knowBroker"), ["knowBroker ws1 codex"]);
        assert_eq!(ctx.called_with("setupRefusalMoved"), ["setupRefusalMoved here claude"]);
    });
}

#[test]
fn agent_lists_answers_a_failed_broker_read_as_a_refusal_and_a_dropped_link_as_the_table() {
    local(async {
        let ctx = FakeAgentsCtx::new();
        ctx.add_adapter("claude", plain_factory(FakeAdapter::new().arc()));
        ctx.add_adapter("codex", plain_factory(FakeAdapter::new().arc()));
        ctx.broker_failures.borrow_mut().insert("codex".to_string(), "no login is held for Codex".to_string());
        let (e, _) = entry(json!({}));
        let listed: Vec<Value> = ctx.area().agent_lists(Some(&e)).await.iter().map(json_of).collect();
        assert!(listed[0].get("refusal").is_none());
        assert_eq!(listed[1]["refusal"], "no login is held for Codex");
        assert_eq!(listed[1]["source"], "table", "the table's lists are the ones the refusal is said beside");

        // A joined computer whose link dropped under the check is its computer's state: the table answers, no refusal.
        let ctx = FakeAgentsCtx::new();
        ctx.add_adapter("claude", plain_factory(FakeAdapter::new().arc()));
        ctx.keep_config_dir("srv", "claude", "/root/claude-wsp");
        ctx.place_rules.borrow_mut().push((
            "srv".to_string(),
            "pwd".to_string(),
            Err(silent("the computer is not connected", "PlaceAbsent")),
        ));
        let (placed, _) = entry(json!({ "kind": "place", "place": "srv" }));
        let listed: Vec<Value> = ctx.area().agent_lists(Some(&placed)).await.iter().map(json_of).collect();
        assert_eq!(listed.len(), 1);
        assert!(listed[0].get("refusal").is_none());
        assert_eq!(listed[0]["source"], "table");
    });
}
