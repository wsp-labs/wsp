// SPDX-License-Identifier: AGPL-3.0-only
//! The pull requests area (spec/pull_requests.md): the git host read for a workspace and the pull request kept on it.
//! Holds runtime invariant 4. Every sentence is the TS's, written by oracle/pull_requests.oracle.ts into
//! tests/fixtures/oracle/pull_requests.json; every frame, record and status is the TS's own literal or what the TS ran
//! to. `World` below is the context: a clock of its own, two daemons that answer by op, and a log of everything the
//! area asked of it, in order (a road is logged when it is asked, a frame when it is sent).
//!
//! Cases: q1 to q16 and the two invariant_4 tests are the spec's rules; each `ts_` test ports a case of
//! test/pull-request.test.ts (or tree.test.ts, worktree-sweep.test.ts) at the area's seam. See spec ## Tests.
#![allow(dead_code, unused_imports)]
mod common;
include!("pull_requests/world.rs");

// ---- Q1

// Q1: this computer answers first; only a no-host-cli refusal falls to a running copy that reads the host itself.
#[test]
fn q1_read_host_falls_back_to_a_running_copy() {
    run(async {
        let read = || builder(|cwd| json!({ "op": "git.prRead", "cwd": cwd, "remote": REMOTE, "branch": "fix/ci" }));
        let here_frame = pr_frame(HOME, json!({ "branch": "fix/ci" }));
        let copy_frame = pr_frame(COPY, json!({ "branch": "fix/ci" }));
        let want = |n: i64| parsed::<GitPrReadReply>(ok(json!({ "pr": open(json!({ "number": n })) })));

        // this computer answers: here is true and the copy is never asked
        let (w, area, e) = setup();
        let got = area.read_host_on(&e, read(), parse_as::<GitPrReadReply>()).await.unwrap();
        assert!(got.here);
        assert_eq!(got.read, want(12));
        assert_eq!(w.log.borrow().clone(), vec!["road here".to_string(), format!("frame here {here_frame}")]);

        // readHost answers the read alone
        let (_w, area, e) = setup();
        assert_eq!(area.read_host(&e, read(), parse_as::<GitPrReadReply>()).await.unwrap(), want(12));

        // no command line here and a running copy: the copy's daemon with the frame built for its checkout
        let (w, area, e) = setup();
        w.here_says("git.prRead", |_| no_gh());
        w.copy_says("git.prRead", |_| ok(json!({ "pr": open(json!({ "number": 3 })) })));
        let got = area.read_host_on(&e, read(), parse_as::<GitPrReadReply>()).await.unwrap();
        assert!(!got.here);
        assert_eq!(got.read, want(3));
        assert_eq!(
            w.log.borrow().clone(),
            vec!["road here".to_string(), format!("frame here {here_frame}"), "road copy".to_string(), format!("frame copy {copy_frame}")]
        );

        // a joined computer's folder is not local: it reads for itself while it runs
        let (w, area, _e) = setup();
        let place = w.entry(json!({ "id": "w2", "kind": "place", "machineId": "", "place": "box1" }));
        w.here_says("git.prRead", |_| no_gh());
        w.copy_says("git.prRead", |_| ok(json!({ "pr": open(json!({ "number": 4 })) })));
        let got = area.read_host_on(&place, read(), parse_as::<GitPrReadReply>()).await.unwrap();
        assert!(!got.here);
        assert_eq!(got.read, want(4));

        // a stopped copy is never woken, a local one has no gh of its own: the refusal stands, the copy is not asked
        for over in [json!({ "phase": "napping" }), json!({ "phase": "pausing" }), json!({ "kind": "local", "machineId": "" })] {
            let (w, area, _e) = setup();
            let entry = w.entry(with(json!({ "id": "w3" }), over.clone()));
            w.here_says("git.prRead", |_| no_gh());
            w.copy_says("git.prRead", |_| ok(json!({ "pr": open(json!({})) })));
            let e = area.read_host_on(&entry, read(), parse_as::<GitPrReadReply>()).await.unwrap_err();
            assert_eq!(refusal(&e), (Some("no-host-cli".into()), NO_GH_TEXT.into()), "{over}");
            assert_eq!(w.roads("copy"), 0, "{over}");
            assert!(w.frames("copy").is_empty(), "{over}");
            assert_eq!(w.frames("here"), vec![here_frame.clone()], "{over}");
        }

        // any other failure rethrows whole, a rate limit and a plain gh failure alike
        for (code, text) in [(Some("rate-limited"), RL_TEXT), (None, GH_DOWN), (Some("no-git-credential"), "no credential")] {
            let (w, area, e) = setup();
            w.here_says("git.prRead", move |_| refused(code, text));
            w.copy_says("git.prRead", |_| ok(json!({ "pr": open(json!({})) })));
            let err = area.read_host_on(&e, read(), parse_as::<GitPrReadReply>()).await.unwrap_err();
            assert_eq!(refusal(&err), (code.map(String::from), text.to_string()));
            assert_eq!(w.roads("copy"), 0);
        }

        // the copy's own failure is what comes back, after both roads were asked
        let (w, area, e) = setup();
        w.here_says("git.prRead", |_| no_gh());
        w.copy_says("git.prRead", |_| refused(None, GH_DOWN));
        let err = area.read_host_on(&e, read(), parse_as::<GitPrReadReply>()).await.unwrap_err();
        assert_eq!(refusal(&err), (None, GH_DOWN.to_string()));
        assert_eq!(w.frames("here"), vec![here_frame.clone()]);
        assert_eq!(w.frames("copy"), vec![copy_frame.clone()]);
    });
}

// Q2: a road that met a rate-limited refusal answers every read with it for a minute and runs nothing.
#[test]
fn q2_rate_limit_holds_the_road() {
    run(async {
        let (_w, area, _e) = setup();
        let runs = Rc::new(Cell::new(0));
        let limit = || refusal_error(Some("rate-limited"), RL_TEXT);
        let first = try_road(&area, &runs, "here", Err(limit())).await.unwrap_err();
        assert_eq!(refusal(&first), (Some("rate-limited".into()), RL_TEXT.into()));
        assert_eq!(runs.get(), 1);
        for _ in 0..3 {
            let held = try_road(&area, &runs, "here", Err(limit())).await.unwrap_err();
            assert_eq!(refusal(&held), (Some("rate-limited".into()), RL_TEXT.into()));
        }
        // a read that would have succeeded is held too
        let held = try_road(&area, &runs, "here", Ok(5)).await.unwrap_err();
        assert_eq!(refusal(&held), (Some("rate-limited".into()), RL_TEXT.into()));
        assert_eq!(runs.get(), 1);
        // another road is its own
        assert_eq!(try_road(&area, &runs, "w1", Ok(5)).await.unwrap(), 5);
        assert_eq!(runs.get(), 2);

        // no other refusal holds anything: each read runs
        let others = [
            refusal_error(Some("no-host-cli"), NO_GH_TEXT),
            refusal_error(None, GH_DOWN),
            refusal_error(Some("rate-limit"), RL_TEXT),
            AccountError::Host(plain(RL_TEXT)),
        ];
        for (n, other) in others.into_iter().enumerate() {
            let road = format!("other{n}");
            let before = runs.get();
            let _ = try_road(&area, &runs, &road, Err(other.clone())).await.unwrap_err();
            let _ = try_road(&area, &runs, &road, Err(other)).await.unwrap_err();
            assert_eq!(runs.get(), before + 2, "{road}");
            assert_eq!(try_road(&area, &runs, &road, Ok(1)).await.unwrap(), 1);
        }
    });
}

// Q2: the hold ends at now >= until, a minute from the refusal, and a read inside it does not move the end.
#[test]
fn q2_the_hold_ends_on_time() {
    run(async {
        let (w, area, _e) = setup();
        let runs = Rc::new(Cell::new(0));
        let limit = || refusal_error(Some("rate-limited"), RL_TEXT);
        let _ = try_road(&area, &runs, "here", Err(limit())).await.unwrap_err();
        w.advance(HOLD / 2);
        let _ = try_road(&area, &runs, "here", Ok(1)).await.unwrap_err();
        w.advance(HOLD / 2 - 1);
        let _ = try_road(&area, &runs, "here", Ok(1)).await.unwrap_err();
        assert_eq!(runs.get(), 1);
        w.advance(1);
        assert_eq!(try_road(&area, &runs, "here", Ok(7)).await.unwrap(), 7);
        assert_eq!(try_road(&area, &runs, "here", Ok(8)).await.unwrap(), 8);
        assert_eq!(runs.get(), 3);
        // a second refusal holds from its own time
        let _ = try_road(&area, &runs, "here", Err(limit())).await.unwrap_err();
        w.advance(HOLD - 1);
        let _ = try_road(&area, &runs, "here", Ok(1)).await.unwrap_err();
        assert_eq!(runs.get(), 4);
        w.advance(1);
        assert_eq!(try_road(&area, &runs, "here", Ok(9)).await.unwrap(), 9);
    });
}

// ---- Q3

// Q3: the entry by entryOf, the pull request's number off the record, the remote off the project; the keys.
#[test]
fn q3_pull_request_on_and_the_keys() {
    run(async {
        let (w, area, e) = setup();
        keep(&e, json!({}));
        let (got, remote, number) = area.pull_request_on("w1", None).await.unwrap();
        assert!(Rc::ptr_eq(&got, &e));
        assert_eq!((remote.as_str(), number), (REMOTE, 12));
        let caller = Caller::Road(parsed::<WorkspaceOrigin>(json!("here")));
        area.pull_request_on("w1", Some(&caller)).await.unwrap();
        assert_eq!(w.log_of("entry_of "), vec!["w1 none".to_string(), "w1 caller".to_string()]);

        // number 0 is a number
        keep(&e, json!({ "number": 0 }));
        assert_eq!(area.pull_request_on("w1", None).await.unwrap().2, 0);

        // no pull request: the plain refusal, by name, before the project is read
        let (w, area, e) = setup();
        set_record(&e, "project", json!("p9"));
        let err = area.pull_request_on("w1", None).await.map(|_| ()).unwrap_err();
        assert_eq!(error_json(&err), oracle("pull_requests")["noPullRequest"]);
        assert!(w.log_of("entry_of ").len() == 1);

        // a pull request on a project nothing holds: the project's own refusal
        keep(&e, json!({}));
        let err = area.pull_request_on("w1", None).await.map(|_| ()).unwrap_err();
        assert_eq!(error_json(&err), json!({ "message": "no project p9" }));

        // an entry nothing holds: entryOf's refusal
        let err = area.pull_request_on("w7", None).await.map(|_| ()).unwrap_err();
        assert_eq!(error_json(&err), json!({ "message": "no workspace w7" }));

        assert_eq!(PullRequestsArea::page_key(REMOTE, 12), "page:https://github.com/wsp/pr-lab.git#12");
        assert_eq!(PullRequestsArea::pr_key(REMOTE, 12), "pr:https://github.com/wsp/pr-lab.git#12");
    });
}

// Q4: a page held PR_PAGE_HOLD_MS per remote and number, dropped by a timer only if still the entry that set it.
#[test]
fn q4_page_held_then_dropped() {
    run(async {
        let (w, area, e) = setup();
        w.here_says("git.prView", |_| ok(page_json()));
        let got = area.read_page(&e, REMOTE, 12, false).await.unwrap();
        assert_eq!(got, ReadOn { read: page(), here: true });
        assert_eq!(w.frames("here"), vec![page_frame(HOME)]);
        let held = w.cached(PAGE_KEY).unwrap();
        assert_eq!(serde_json::to_string(&held.body).unwrap(), page_body(true));
        assert_eq!((held.tag, held.fetched_at), (None, T0));
        w.assert_due(&[T0 + PAGE_HOLD]);

        // within the hold nothing is read
        w.advance(PAGE_HOLD - 1);
        assert_eq!(area.read_page(&e, REMOTE, 12, false).await.unwrap(), ReadOn { read: page(), here: true });
        assert_eq!(w.frames("here").len(), 1);
        // at the hold the timer drops it and the next read asks again
        w.advance(1);
        assert!(w.cached(PAGE_KEY).is_none());
        area.read_page(&e, REMOTE, 12, false).await.unwrap();
        assert_eq!(w.frames("here").len(), 2);
        assert_eq!(w.cached(PAGE_KEY).unwrap().fetched_at, T0 + PAGE_HOLD);
        // a fresh read asks even while held
        area.read_page(&e, REMOTE, 12, true).await.unwrap();
        assert_eq!(w.frames("here").len(), 3);
        // another number is its own page
        area.read_page(&e, REMOTE, 13, false).await.unwrap();
        assert_eq!(
            w.frames("here").last().unwrap(),
            &json!({ "op": "git.prView", "cwd": HOME, "remote": REMOTE, "number": 13 }).to_string()
        );
    });
}

// Q4: the timer of an older read leaves an entry a fresh read set after it.
#[test]
fn q4_an_old_timer_leaves_a_newer_entry() {
    run(async {
        let (w, area, e) = setup();
        w.here_says("git.prView", |_| ok(page_json()));
        area.read_page(&e, REMOTE, 12, false).await.unwrap();
        w.advance(PAGE_HOLD / 2);
        area.read_page(&e, REMOTE, 12, true).await.unwrap();
        assert_eq!(w.frames("here").len(), 2);
        w.advance(PAGE_HOLD / 2);
        assert_eq!(w.cached(PAGE_KEY).unwrap().fetched_at, T0 + PAGE_HOLD / 2);
        area.read_page(&e, REMOTE, 12, false).await.unwrap();
        assert_eq!(w.frames("here").len(), 2);
        w.advance(PAGE_HOLD / 2);
        assert!(w.cached(PAGE_KEY).is_none());
    });
}

// Q4: a page read by the copy says so, and is held as read by it.
#[test]
fn q4_a_page_the_copy_read_says_here_false() {
    run(async {
        let (w, area, e) = setup();
        w.here_says("git.prView", |_| no_gh());
        w.copy_says("git.prView", |_| ok(page_json()));
        let got = area.read_page(&e, REMOTE, 12, false).await.unwrap();
        assert_eq!(got, ReadOn { read: page(), here: false });
        assert_eq!(w.frames("here"), vec![page_frame(HOME)]);
        assert_eq!(w.frames("copy"), vec![page_frame(COPY)]);
        assert_eq!(serde_json::to_string(&w.cached(PAGE_KEY).unwrap().body).unwrap(), page_body(false));
        assert_eq!(area.read_page(&e, REMOTE, 12, false).await.unwrap(), ReadOn { read: page(), here: false });
        assert_eq!(w.frames("copy").len(), 1);
        // a read that failed holds nothing
        let (w, area, e) = setup();
        w.here_says("git.prView", |_| refused(None, GH_DOWN));
        let err = area.read_page(&e, REMOTE, 12, false).await.map(|_| ()).unwrap_err();
        assert_eq!(refusal(&err), (None, GH_DOWN.to_string()));
        assert!(w.cached(PAGE_KEY).is_none());
        w.assert_due(&[]);
    });
}

// Invariant 4: nothing posts as the person through a copy's gh. This computer has no command line for the host, the
// only live copy runs and would answer every write, and the copy's road is never asked: not a frame, not the road.
#[test]
fn invariant_4_post_as_person_never_takes_the_copy_road() {
    run(async {
        let (w, area, _e) = setup();
        let line = oracle("pull_requests")["posts"][0]["line"].clone();
        for (op, _) in writes() {
            w.here_says(op, |_| no_gh());
            w.copy_says(op, |f| ok(json!({ "op": f["op"].clone(), "posted": "through the copy" })));
        }
        for (op, frame_for) in writes() {
            let err = area.post_as_person(REMOTE, 12, frame_for, whole()).await.unwrap_err();
            assert_eq!(host_json(&err), json!({ "message": line }), "{op}");
        }
        let key = PAGE_KEY.to_string();
        let mut expected = vec![];
        for op in ["git.prReply", "git.prResolve", "git.prReact"] {
            expected.push(format!("cache.delete {key}"));
            expected.push("road here".to_string());
            expected.push(format!("frame here {}", write_frame(op)));
        }
        assert_eq!(w.log.borrow().clone(), expected);
        assert_eq!(w.roads("copy"), 0);
        assert!(w.frames("copy").is_empty());
    });
}

// Invariant 4: the held page goes first, before this computer is asked and whether or not the write lands.
#[test]
fn invariant_4_post_as_person_drops_the_held_page() {
    run(async {
        let (w, area, e) = setup();
        w.here_says("git.prView", |_| ok(page_json()));
        w.here_says("git.prReact", |f| ok(json!({ "subject": f["subject"].clone(), "reactions": [] })));
        area.read_page(&e, REMOTE, 12, false).await.unwrap();
        w.cache.borrow_mut().insert(
            "pr:https://github.com/wsp/pr-lab.git#12".into(),
            CacheEntry { body: json!({}), tag: Some(SEEN.into()), fetched_at: T0 },
        );
        w.cache
            .borrow_mut()
            .insert("page:https://github.com/wsp/other.git#12".into(), CacheEntry { body: json!({}), tag: None, fetched_at: T0 });
        w.log.borrow_mut().clear();
        let reply = area.post_as_person(REMOTE, 12, writes().pop().unwrap().1, whole()).await.unwrap();
        assert_eq!(Value::Object(reply), ok(json!({ "subject": "IC_kwDOx", "reactions": [] })));
        assert_eq!(
            w.log.borrow().clone(),
            vec![format!("cache.delete {PAGE_KEY}"), "road here".to_string(), format!("frame here {}", write_frame("git.prReact"))]
        );
        let mut left: Vec<String> = w.cache.borrow().keys().cloned().collect();
        left.sort();
        assert_eq!(
            left,
            vec!["page:https://github.com/wsp/other.git#12".to_string(), "pr:https://github.com/wsp/pr-lab.git#12".to_string()]
        );
        // the next view reads again
        area.read_page(&e, REMOTE, 12, false).await.unwrap();
        assert_eq!(w.frames("here").len(), 2);

        // a write this computer refuses drops the page all the same
        w.here_says("git.prReact", |_| no_gh());
        w.log.borrow_mut().clear();
        let err = area.post_as_person(REMOTE, 12, writes().pop().unwrap().1, whole()).await.unwrap_err();
        assert_eq!(host_json(&err), json!({ "message": oracle("pull_requests")["posts"][0]["line"].clone() }));
        assert!(w.cached(PAGE_KEY).is_none());
        assert_eq!(w.log_of("cache.delete "), vec![PAGE_KEY.to_string()]);
        assert_eq!(w.log.borrow()[0], format!("cache.delete {PAGE_KEY}"));
    });
}

// Q5: any other failure rethrows whole; the rate-limit hold of the read road does not stop a write.
#[test]
fn q5_post_as_person_rethrows_other_failures_and_ignores_the_hold() {
    run(async {
        let (w, area, _e) = setup();
        w.here_says("git.prReact", |_| refused(Some("forbidden"), "gh said: Resource not accessible"));
        let err = area.post_as_person(REMOTE, 12, writes().pop().unwrap().1, whole()).await.unwrap_err();
        assert_eq!(refusal(&err), (Some("forbidden".into()), "gh said: Resource not accessible".into()));
        assert_eq!(w.roads("copy"), 0);

        let runs = Rc::new(Cell::new(0));
        let _ = try_road(&area, &runs, "here", Err(refusal_error(Some("rate-limited"), RL_TEXT))).await.unwrap_err();
        w.here_says("git.prReact", |f| {
            ok(json!({ "subject": f["subject"].clone(), "reactions": [{ "content": "eyes", "count": 1, "mine": true }] }))
        });
        w.log.borrow_mut().clear();
        let reply = area.post_as_person(REMOTE, 12, writes().pop().unwrap().1, whole()).await.unwrap();
        assert_eq!(
            Value::Object(reply),
            ok(json!({ "subject": "IC_kwDOx", "reactions": [{ "content": "eyes", "count": 1, "mine": true }] }))
        );
        assert_eq!(w.frames("here"), vec![write_frame("git.prReact")]);

        // a reply that does not parse is the parse's own failure
        let bad: Rc<dyn Fn(Frame) -> AccountResult<Frame>> = Rc::new(|_| Err(plain("not a reply").into()));
        let err = area.post_as_person(REMOTE, 12, writes().pop().unwrap().1, bad).await.unwrap_err();
        assert_eq!(host_json(&err), json!({ "message": "not a reply" }));
    });
}

// Q5: hostOfRemote: the git host's first ssh host, else the remote's own host in lower case, else the remote, as the
// post line each shape of remote gets (the TS lower-cases only after its scheme strip, so an upper-case scheme is a host).
#[test]
fn q5_host_of_remote_and_the_post_line() {
    run(async {
        let posts = oracle("pull_requests")["posts"].clone();
        for p in posts.as_array().unwrap() {
            let remote = p["remote"].as_str().unwrap();
            assert_eq!(PullRequestsArea::host_of_remote(remote), p["host"].as_str().unwrap(), "{remote}");
            let (w, area, _e) = setup();
            w.here_says("git.prReact", |_| no_gh());
            let err = area.post_as_person(remote, 12, writes().pop().unwrap().1, whole()).await.unwrap_err();
            assert_eq!(host_json(&err), json!({ "message": p["line"].clone() }), "{remote}");
        }
    });
}

// ---- Q6

// Q6: PR_POLL_MS while a window watches, PR_POLL_IDLE_MS otherwise.
#[test]
fn q6_pr_poll_ms_follows_the_window() {
    let (w, area, _e) = setup();
    assert_eq!(area.pr_poll_ms(), IDLE);
    w.watched.set(true);
    assert_eq!(area.pr_poll_ms(), POLL);
    w.watched.set(false);
    assert_eq!(area.pr_poll_ms(), IDLE);
}

// Q7: the table, one case per step of the spec.
#[test]
fn q7_read_pull_request_table() {
    run(async {
        q7_left_branch_drops_and_reads_again().await;
        q7_a_settled_record_is_never_read().await;
        q7_a_held_read_answers_held().await;
        q7_a_branch_with_no_pull_request_waits_for_its_head().await;
        q7_the_in_flight_mark().await;
        q7_the_read_frame_and_when_none_is_sent().await;
        q7_what_is_taken_from_the_answer().await;
        q7_a_failed_read().await;
    });
}

// Q7 step 5: a second reader joins the read in flight: one frame, one take, the same answer.
#[test]
fn q7_a_read_in_flight_is_joined() {
    run(async {
        let (w, area, e) = setup();
        w.on_branch(&e, "fix/ci", "h1");
        w.here.hold.set(true);
        let (a1, e1) = (area.clone(), e.clone());
        let first = tokio::task::spawn_local(async move { a1.read_pull_request(&e1, true, false).await });
        settle().await;
        assert_eq!(w.frames("here").len(), 1);
        let (a2, e2) = (area.clone(), e.clone());
        let second = tokio::task::spawn_local(async move { a2.read_pull_request(&e2, true, false).await });
        settle().await;
        assert_eq!(w.frames("here").len(), 1);
        assert!(e.borrow().pr_reading.is_some());
        w.release("here");
        settle().await;
        let (one, two) = (first.await.unwrap(), second.await.unwrap());
        assert_eq!(pr_value(&one), fact(json!({}), T0));
        assert_eq!(pr_value(&two), fact(json!({}), T0));
        assert_eq!(w.frames("here"), vec![pr_frame(HOME, json!({ "branch": "fix/ci" }))]);
        assert_eq!(w.statuses.borrow().len(), 1);
        assert!(e.borrow().pr_reading.is_none());
    });
}

// Q7 step 5: a whole read does not join one in flight, and never sends what the last read saw.
#[test]
fn q7_whole_reads_alone() {
    run(async {
        let (w, area, e) = setup();
        keep(&e, json!({}));
        w.on_branch(&e, "fix/ci", "h1");
        w.cache
            .borrow_mut()
            .insert(PullRequestsArea::pr_key(REMOTE, 12), CacheEntry { body: open(json!({})), tag: Some(SEEN.into()), fetched_at: T0 - 5 });
        w.here_says("git.prRead", |f| {
            if f.contains_key("seen") {
                ok(json!({ "unchanged": true, "seen": SEEN }))
            } else {
                ok(json!({ "pr": open(json!({ "checks": [{ "name": "ci", "state": "fail" }] })), "seen": SEEN }))
            }
        });
        w.here.hold.set(true);
        let (a1, e1) = (area.clone(), e.clone());
        let first = tokio::task::spawn_local(async move { a1.read_pull_request(&e1, true, false).await });
        settle().await;
        let (a2, e2) = (area.clone(), e.clone());
        let second = tokio::task::spawn_local(async move { a2.read_pull_request(&e2, true, true).await });
        settle().await;
        assert_eq!(w.frames("here"), vec![pr_frame(HOME, json!({ "number": 12, "seen": SEEN })), pr_frame(HOME, json!({ "number": 12 }))]);
        w.release("here");
        settle().await;
        let (one, two) = (first.await.unwrap(), second.await.unwrap());
        assert_eq!(pr_value(&one)["checks"][0]["state"], json!("pass"));
        assert_eq!(pr_value(&two)["checks"][0]["state"], json!("fail"));
        assert!(e.borrow().pr_reading.is_none());
    });
}

// Q8: the record is written only where state, number or branch moved, in the TS's key order; checks only once settled.
#[test]
fn q8_take_writes_the_record_only_when_it_moved() {
    run(async {
        let (w, area, e) = setup();
        let written = |state_keys: Value| kept_pr(state_keys).to_string();

        // the first read of an open one writes it, without checks
        area.take_pull_request(&e, Some(seen_of(fact(json!({}), T0)))).await.unwrap();
        assert_eq!(record_pr(&e), written(json!({})));
        assert_eq!(w.fields_persisted(), vec![format!("{{\"pr\":{}}}", written(json!({})))]);
        assert_eq!(pr_of(&e), fact(json!({}), T0));
        assert_eq!(w.log.borrow().clone(), vec!["persist".to_string(), "status".to_string()]);
        w.assert_due(&[T0 + IDLE]);

        // the same state, number and branch again: no write, the status pushed, the timer moved
        w.advance(1000);
        area.take_pull_request(&e, Some(seen_of(fact(json!({ "headOid": "c0ffee" }), T0 + 1000)))).await.unwrap();
        assert_eq!(w.persisted.borrow().len(), 1);
        assert_eq!(pr_of(&e), fact(json!({ "headOid": "c0ffee" }), T0 + 1000));
        assert_eq!(w.statuses.borrow().len(), 2);
        w.assert_due(&[T0 + 1000 + IDLE]);

        // a move of branch alone writes
        area.take_pull_request(&e, Some(seen_of(fact(json!({ "branch": "fix/two" }), T0 + 1000)))).await.unwrap();
        assert_eq!(w.persisted.borrow().len(), 2);
        assert_eq!(record_pr(&e), kept_pr(json!({ "branch": "fix/two" })).to_string());

        // merged: mergedAt is the read's time, checks come with it, in that order; the timer goes; the tree settles
        let checks = json!([{ "name": "ci", "workflow": "ci", "state": "pass", "startedAt": "2026-10-02T10:00:00Z", "completedAt": "2026-10-02T10:16:00Z" }]);
        area.take_pull_request(&e, Some(seen_of(fact(json!({ "state": "merged", "checks": checks.clone() }), T0 + 2000)))).await.unwrap();
        let merged = json!({ "number": 12, "url": "https://github.com/wsp/pr-lab/pull/12", "state": "merged", "base": "main", "branch": "fix/ci", "mergedAt": T0 + 2000, "checks": checks.clone() });
        assert_eq!(record_pr(&e), merged.to_string());
        assert_eq!(w.persisted.borrow().len(), 3);
        w.assert_due(&[]);
        assert!(e.borrow().pr_poll.is_none());

        // closed: closedAt, not mergedAt
        let (w, area, e) = setup();
        area.take_pull_request(&e, Some(seen_of(fact(json!({ "state": "closed" }), T0)))).await.unwrap();
        let closed = json!({ "number": 12, "url": "https://github.com/wsp/pr-lab/pull/12", "state": "closed", "base": "main", "branch": "fix/ci", "closedAt": T0, "checks": open(json!({}))["checks"].clone() });
        assert_eq!(record_pr(&e), closed.to_string());
        w.assert_due(&[]);

        // nothing, a why and a kept one set the entry and the status and touch no record
        for seen in [
            None,
            Some(seen_of(why("a sentence", T0))),
            Some(seen_of(json!({ "number": 12, "url": "u", "state": "open", "base": "main", "readAt": T0 }))),
        ] {
            let (w, area, e) = setup();
            keep(&e, json!({}));
            let want = pr_value(&seen);
            area.take_pull_request(&e, seen).await.unwrap();
            assert_eq!(pr_of(&e), want);
            assert_eq!(w.statuses.borrow().clone(), vec![want]);
            assert!(w.persisted.borrow().is_empty());
            assert_eq!(record_pr(&e), kept_pr(json!({})).to_string());
            w.assert_due(&[]);
        }

        // a settled record settles the tree even when this take was not the one that settled it
        let (w, area, e) = setup();
        keep(&e, json!({ "state": "merged", "mergedAt": T0 - 1 }));
        w.views.borrow_mut().push(sv("s1", "w1", Some("tA"), None, "completed"));
        area.take_pull_request(&e, None).await.unwrap();
        assert_eq!(w.log_of("mark "), vec![format!("[\"tA\"] {T0}")]);
    });
}

// Q8: what was sent belongs to the pull request it was read off.
#[test]
fn q8_a_new_number_drops_what_was_sent() {
    run(async {
        let (w, area, e) = setup();
        let sent = json!([{ "kind": "comment", "id": 5841958969_i64, "at": T0 }]);
        keep(&e, json!({}));
        set_record(&e, "prSent", sent.clone());
        // the same pull request read again keeps it
        area.take_pull_request(&e, Some(seen_of(fact(json!({ "state": "merged" }), T0)))).await.unwrap();
        assert_eq!(rec_of(&e)["prSent"], sent);
        // another number starts with nothing sent
        area.take_pull_request(&e, Some(seen_of(fact(json!({ "number": 13, "url": "https://github.com/wsp/pr-lab/pull/13" }), T0))))
            .await
            .unwrap();
        assert!(rec_of(&e).get("prSent").is_none());
        assert_eq!(record_pr(&e), kept_pr(json!({ "number": 13, "url": "https://github.com/wsp/pr-lab/pull/13" })).to_string());
        let last = w.fields_persisted().pop().unwrap();
        assert_eq!(last, format!("{{\"pr\":{}}}", kept_pr(json!({ "number": 13, "url": "https://github.com/wsp/pr-lab/pull/13" }))));

        // with no pull request on the record yet there is nothing it belonged to: what was sent stays
        let (_w, area, e) = setup();
        set_record(&e, "prSent", sent.clone());
        area.take_pull_request(&e, Some(seen_of(fact(json!({}), T0)))).await.unwrap();
        assert_eq!(rec_of(&e)["prSent"], sent);
    });
}

// ---- Q9

// Q9: left_branch: only a shared folder with a kept pr and a checkout on a real branch.
#[test]
fn q9_left_branch_table() {
    // (label, record over, project base, project default branch, kept pr, checkout branch, left)
    let folder = json!({ "kind": "local", "machineId": "" });
    let place = json!({ "kind": "place", "machineId": "", "place": "box1" });
    let cloud = json!({});
    let worktree =
        json!({ "kind": "local", "machineId": "", "worktree": { "path": "/home/u/.wsp/trees/x", "branch": "fix/ci", "made": true } });
    let on_branch = Some(kept_pr(json!({})));
    let no_branch = Some(json!({ "number": 12, "url": "https://github.com/wsp/pr-lab/pull/12", "state": "open", "base": "main" }));
    let rows: Vec<(&str, Value, Option<&str>, &str, Option<Value>, Option<&str>, bool)> = vec![
        ("on its branch", folder.clone(), Some("main"), "main", on_branch.clone(), Some("fix/ci"), false),
        ("back on the base", folder.clone(), Some("main"), "main", on_branch.clone(), Some("main"), true),
        ("on another branch", folder.clone(), Some("main"), "main", on_branch.clone(), Some("other"), true),
        ("a joined computer's folder", place.clone(), Some("main"), "main", on_branch.clone(), Some("main"), true),
        ("a machine keeps its pr", cloud.clone(), Some("main"), "main", on_branch.clone(), Some("main"), false),
        ("a worktree keeps its pr", worktree.clone(), Some("main"), "main", on_branch.clone(), Some("main"), false),
        ("no pull request", folder.clone(), Some("main"), "main", None, Some("main"), false),
        ("no checkout read", folder.clone(), Some("main"), "main", on_branch.clone(), None, false),
        ("detached", folder.clone(), Some("main"), "main", on_branch.clone(), Some("(HEAD detached at abc1234)"), false),
        ("rebasing", folder.clone(), Some("main"), "main", on_branch.clone(), Some("(no branch, rebasing fix/ci)"), false),
        ("kept before branches, on the base", folder.clone(), Some("main"), "main", no_branch.clone(), Some("main"), true),
        ("kept before branches, on a branch", folder.clone(), Some("main"), "main", no_branch.clone(), Some("feature/x"), false),
        (
            "the record's base outranks",
            with(folder.clone(), json!({ "base": "develop" })),
            Some("main"),
            "main",
            no_branch.clone(),
            Some("develop"),
            true,
        ),
        (
            "the record's base, project's is not it",
            with(folder.clone(), json!({ "base": "develop" })),
            Some("main"),
            "main",
            no_branch.clone(),
            Some("main"),
            false,
        ),
        ("the project's base", folder.clone(), Some("trunk"), "main", no_branch.clone(), Some("trunk"), true),
        ("the project's base, default is not it", folder.clone(), Some("trunk"), "main", no_branch.clone(), Some("main"), false),
        ("the default branch when none is named", folder.clone(), None, "dev", no_branch.clone(), Some("dev"), true),
        ("the default branch, main is not it", folder.clone(), None, "dev", no_branch.clone(), Some("main"), false),
    ];
    for (label, over, base, default_branch, kept, branch, left) in rows {
        let (w, area, _e) = setup();
        w.set_project(base, default_branch, REMOTE);
        let e = w.entry(with(json!({ "id": "w9" }), over));
        if let Some(k) = kept {
            let mut v = rec_of(&e);
            v["pr"] = k;
            *e.borrow().record.borrow_mut() = parsed(v);
        }
        if let Some(b) = branch {
            e.borrow_mut().checkout = Some(checkout_json(b, "h1"));
        }
        assert_eq!(area.left_branch(&e), left, "{label}");
    }
}

// ---- Q10

// Q10: the poll, the entry's pr, the head mark, the record's pr and what was sent all go, then the record is kept and
// the status pushed.
#[test]
fn q10_drop_pull_request_clears_everything() {
    run(async {
        let (w, area, e) = setup();
        keep(&e, json!({}));
        set_record(&e, "prSent", json!([{ "kind": "comment", "id": 1, "at": T0 }]));
        set_record(&e, "branchThreads", json!({ "branch": "fix/ci", "threads": ["tA"] }));
        e.borrow_mut().pr = Some(seen_of(fact(json!({}), T0)));
        e.borrow_mut().pr_none_at = Some("h1".into());
        let (cancelled, cancel) = counted();
        e.borrow_mut().pr_poll = held(cancel);
        area.drop_pull_request(&e).await.unwrap();
        assert_eq!(cancelled.get(), 1);
        assert!(e.borrow().pr_poll.is_none());
        assert_eq!(pr_of(&e), Value::Null);
        assert_eq!(e.borrow().pr_none_at.clone(), None);
        assert_eq!(w.fields_persisted(), vec!["{\"branchThreads\":{\"branch\":\"fix/ci\",\"threads\":[\"tA\"]}}".to_string()]);
        assert_eq!(w.log.borrow().clone(), vec!["persist".to_string(), "status".to_string()]);
        assert_eq!(w.statuses.borrow().clone(), vec![Value::Null]);
        // with nothing kept it still persists and pushes
        area.drop_pull_request(&e).await.unwrap();
        assert_eq!(w.log.borrow().len(), 4);
    });
}

// ---- Q11

// Q11: only a shared folder on a real branch; its base keeps nothing; another branch starts a new list.
#[test]
fn q11_ran_on_branch_table() {
    run(async {
        let (w, area, _e) = setup();
        let e = w.folder();
        let kept = |e: &Entry| rec_of(e).get("branchThreads").map(|v| v.to_string()).unwrap_or_else(|| "none".into());

        // on its base, with nothing kept: nothing is written
        w.on_branch(&e, "main", "h1");
        area.ran_on_branch(&e, "tC").await.unwrap();
        assert!(w.persisted.borrow().is_empty());
        assert_eq!(kept(&e), "none");

        w.on_branch(&e, "fix/ci", "h1");
        area.ran_on_branch(&e, "tA").await.unwrap();
        assert_eq!(kept(&e), json!({ "branch": "fix/ci", "threads": ["tA"] }).to_string());
        assert_eq!(w.persisted.borrow().len(), 1);
        // the same thread again moves nothing
        area.ran_on_branch(&e, "tA").await.unwrap();
        assert_eq!(w.persisted.borrow().len(), 1);
        area.ran_on_branch(&e, "tB").await.unwrap();
        assert_eq!(kept(&e), json!({ "branch": "fix/ci", "threads": ["tA", "tB"] }).to_string());
        assert_eq!(w.persisted.borrow().len(), 2);
        assert_eq!(
            w.fields_persisted().last().unwrap(),
            &format!("{{\"branchThreads\":{}}}", json!({ "branch": "fix/ci", "threads": ["tA", "tB"] }))
        );
        // a branch changed starts a new list
        w.on_branch(&e, "fix/two", "h2");
        area.ran_on_branch(&e, "tC").await.unwrap();
        assert_eq!(kept(&e), json!({ "branch": "fix/two", "threads": ["tC"] }).to_string());
        assert_eq!(w.persisted.borrow().len(), 3);
        // a (-branch says nothing
        w.on_branch(&e, "(HEAD detached at abc1234)", "h3");
        area.ran_on_branch(&e, "tD").await.unwrap();
        assert_eq!(kept(&e), json!({ "branch": "fix/two", "threads": ["tC"] }).to_string());
        assert_eq!(w.persisted.borrow().len(), 3);
        // back on its base removes what was kept, and persists once
        w.on_branch(&e, "main", "h4");
        area.ran_on_branch(&e, "tE").await.unwrap();
        assert_eq!(kept(&e), "none");
        assert_eq!(w.persisted.borrow().len(), 4);
        assert_eq!(w.fields_persisted().last().unwrap(), "{}");
        area.ran_on_branch(&e, "tE").await.unwrap();
        assert_eq!(w.persisted.borrow().len(), 4);

        // a record's own base is the base
        set_record(&e, "base", json!("develop"));
        w.on_branch(&e, "main", "h5");
        area.ran_on_branch(&e, "tF").await.unwrap();
        assert_eq!(kept(&e), json!({ "branch": "main", "threads": ["tF"] }).to_string());

        // a worktree, a machine and a checkout never read keep nothing
        let tree = w
            .entry(json!({ "id": "w5", "kind": "local", "machineId": "", "worktree": { "path": "/p", "branch": "fix/ci", "made": true } }));
        let cloud = w.entry(json!({ "id": "w6" }));
        let bare = w.folder();
        w.on_branch(&tree, "fix/ci", "h1");
        w.on_branch(&cloud, "fix/ci", "h1");
        let before = w.persisted.borrow().len();
        area.ran_on_branch(&tree, "tA").await.unwrap();
        area.ran_on_branch(&cloud, "tA").await.unwrap();
        area.ran_on_branch(&bare, "tA").await.unwrap();
        assert_eq!(w.persisted.borrow().len(), before);
        assert_eq!((kept(&tree), kept(&cloud)), ("none".to_string(), "none".to_string()));
    });
}

// ---- Q12

// Q12: the poll arms only for an open fact on an entry still live; its interval and its first moment.
#[test]
fn q12_poll_intervals() {
    run(async {
        let pending_fact = fact(json!({ "checks": [{ "name": "ci", "state": "pending" }] }), T0);
        // open, no window, no slate: the idle interval
        let (w, area, e) = setup();
        e.borrow_mut().pr = Some(seen_of(fact(json!({}), T0)));
        area.poll_pull_request(&e, None);
        w.assert_due(&[T0 + IDLE]);
        assert_eq!(e.borrow().pr_poll_ms, Some(IDLE as u64));
        assert!(e.borrow().pr_poll.is_some());
        // arming again cancels the first
        area.poll_pull_request(&e, None);
        w.assert_due(&[T0 + IDLE]);
        // a window: the watched interval
        w.watched.set(true);
        area.poll_pull_request(&e, None);
        w.assert_due(&[T0 + POLL]);
        assert_eq!(e.borrow().pr_poll_ms, Some(POLL as u64));

        // a pending check read by a slate: thirty seconds, window or none; a slate with nothing pending, or
        // pending with no slate, keeps the pace of the window
        for (watched, pending, slate, ms) in [
            (false, true, true, SLATE_POLL),
            (true, true, true, SLATE_POLL),
            (false, true, false, IDLE),
            (true, true, false, POLL),
            (false, false, true, IDLE),
            (true, false, true, POLL),
        ] {
            let (w, area, e) = setup();
            w.watched.set(watched);
            w.slate_watches.set(slate);
            let f = if pending { pending_fact.clone() } else { fact(json!({}), T0) };
            e.borrow_mut().pr = Some(seen_of(f));
            area.poll_pull_request(&e, None);
            w.assert_due(&[T0 + ms]);
            assert_eq!(e.borrow().pr_poll_ms, Some(ms as u64));
        }

        // from: the interval counts from the last read; one already past fires at once
        let (w, area, e) = setup();
        e.borrow_mut().pr = Some(seen_of(fact(json!({}), T0)));
        area.poll_pull_request(&e, Some(T0 - 100_000));
        w.assert_due(&[T0 + IDLE - 100_000]);
        assert_eq!(e.borrow().pr_poll_ms, Some(IDLE as u64));
        area.poll_pull_request(&e, Some(T0 - 2 * IDLE));
        w.assert_due(&[T0]);

        // nothing is armed for anything but an open fact, and what was armed is cancelled
        for held in [
            Some(seen_of(fact(json!({ "state": "merged" }), T0))),
            Some(seen_of(fact(json!({ "state": "closed" }), T0))),
            Some(seen_of(why("w", T0))),
            None,
        ] {
            let (w, area, e) = setup();
            e.borrow_mut().pr = Some(seen_of(fact(json!({}), T0)));
            area.poll_pull_request(&e, None);
            assert_eq!(w.clock.pending(), 1);
            e.borrow_mut().pr = held;
            area.poll_pull_request(&e, None);
            w.assert_due(&[]);
            assert!(e.borrow().pr_poll.is_none());
        }

        // nor for an entry the live map no longer holds under its id, or holds another under
        let (w, area, e) = setup();
        e.borrow_mut().pr = Some(seen_of(fact(json!({}), T0)));
        w.live.borrow_mut().shift_remove("w1");
        area.poll_pull_request(&e, None);
        w.assert_due(&[]);
        let other = w.cloud();
        assert!(!Rc::ptr_eq(&other, &e));
        area.poll_pull_request(&e, None);
        w.assert_due(&[]);
        assert!(e.borrow().pr_poll.is_none());
    });
}

// Q12: the timer clears itself and forces a read, which re-arms it.
#[test]
fn q12_the_timer_forces_a_read() {
    run(async {
        let (w, area, e) = setup();
        keep(&e, json!({}));
        w.on_branch(&e, "fix/ci", "h1");
        e.borrow_mut().pr = Some(seen_of(fact(json!({}), T0)));
        area.poll_pull_request(&e, None);
        w.advance(IDLE - 1);
        settle().await;
        assert!(w.frames("here").is_empty());
        w.advance(1);
        settle().await;
        assert_eq!(w.frames("here"), vec![pr_frame(HOME, json!({ "number": 12 }))]);
        assert_eq!(pr_of(&e), fact(json!({}), T0 + IDLE));
        w.assert_due(&[T0 + 2 * IDLE]);
    });
}

// Q13: the roots are the workspace's rows with no root thread; the tree is the roots and every row under one, across
// workspaces; its unique keys are marked read and settled now, and a machine that pauses is napped. Nothing is deleted.
#[test]
fn q13_settle_marks_the_tree_and_naps() {
    run(async {
        let (w, area, e) = setup();
        keep(&e, settled_pr());
        tree_views(&w);
        w.pauses.set(true);
        area.settle_tree(&e).await;
        settle().await;
        assert_eq!(w.log.borrow().clone(), vec![format!("mark [\"tA\",\"tB\",\"s4\"] {T0}"), "nap w1".to_string()]);
        assert!(w.persisted.borrow().is_empty());
        assert!(w.statuses.borrow().is_empty());

        // a machine that does not pause, or is not running, is not napped
        for (pauses, phase) in [(false, "running"), (true, "napping"), (true, "pausing")] {
            let (w, area, _e) = setup();
            let e = w.entry(json!({ "id": "w1", "phase": phase }));
            keep(&e, settled_pr());
            tree_views(&w);
            w.pauses.set(pauses);
            area.settle_tree(&e).await;
            settle().await;
            assert_eq!(w.log.borrow().clone(), vec![format!("mark [\"tA\",\"tB\",\"s4\"] {T0}")], "{pauses} {phase}");
        }

        // no roots: nothing; a workspace a thread opened: nothing
        let (w, area, e) = setup();
        keep(&e, settled_pr());
        w.views.borrow_mut().push(sv("s9", "w1", Some("tK"), Some("tOther"), "completed"));
        w.pauses.set(true);
        area.settle_tree(&e).await;
        settle().await;
        assert!(w.log.borrow().is_empty());
        let (w, area, e) = setup();
        keep(&e, settled_pr());
        set_record(&e, "parentThreadId", json!("tL"));
        tree_views(&w);
        w.pauses.set(true);
        area.settle_tree(&e).await;
        settle().await;
        assert!(w.log.borrow().is_empty());
    });
}
