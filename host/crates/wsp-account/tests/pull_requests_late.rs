// SPDX-License-Identifier: AGPL-3.0-only
//! Part 2 of `pull_requests.rs`: the same world, the tests after it.
#![allow(dead_code, unused_imports)]

mod common;
include!("pull_requests/world.rs");

// Q13: any running turn in the tree, on any workspace it spans, leaves it whole; a running row outside it does not.
#[test]
fn q13_a_running_turn_waits() {
    run(async {
        let (w, area, e) = setup();
        keep(&e, settled_pr());
        w.pauses.set(true);
        w.views.borrow_mut().push(sv("s1", "w1", Some("tA"), None, "completed"));
        w.views.borrow_mut().push(sv("s2", "w2", Some("tB"), Some("tA"), "running"));
        area.settle_tree(&e).await;
        settle().await;
        assert!(w.log.borrow().is_empty());
        // the child's turn ends and its end settles the tree
        w.views.borrow_mut()[1] = sv("s2", "w2", Some("tB"), Some("tA"), "completed");
        area.settle_tree(&e).await;
        settle().await;
        assert_eq!(w.log.borrow().clone(), vec![format!("mark [\"tA\",\"tB\"] {T0}"), "nap w1".to_string()]);
        // a running root of the tree itself waits too
        let (w, area, e) = setup();
        keep(&e, settled_pr());
        w.views.borrow_mut().push(sv("s1", "w1", Some("tA"), None, "running"));
        area.settle_tree(&e).await;
        assert!(w.log.borrow().is_empty());
        // a running row of another tree does not
        let (w, area, e) = setup();
        keep(&e, settled_pr());
        w.views.borrow_mut().push(sv("s1", "w1", Some("tA"), None, "completed"));
        w.views.borrow_mut().push(sv("s2", "w3", Some("tZ"), None, "running"));
        area.settle_tree(&e).await;
        assert_eq!(w.log_of("mark "), vec![format!("[\"tA\"] {T0}")]);
    });
}

// Q13: in a shared folder only the threads kept for the pull request's branch settle.
#[test]
fn q13_a_folder_settles_only_its_branch_threads() {
    run(async {
        let rows = |w: &World| {
            let mut v = w.views.borrow_mut();
            v.push(sv("s1", "w1", Some("tA"), None, "completed"));
            v.push(sv("s2", "w1", Some("tB"), None, "completed"));
            v.push(sv("s3", "w1", Some("tC"), None, "completed"));
            v.push(sv("s4", "w1", Some("tD"), Some("tA"), "completed"));
        };
        // the thread that ran there, and what is under it
        let (w, area, _e) = setup();
        let e = w.folder();
        keep(&e, settled_pr());
        set_record(&e, "branchThreads", json!({ "branch": "fix/ci", "threads": ["tA"] }));
        rows(&w);
        area.settle_tree(&e).await;
        assert_eq!(w.log_of("mark "), vec![format!("[\"tA\",\"tD\"] {T0}")]);

        // several
        set_record(&e, "branchThreads", json!({ "branch": "fix/ci", "threads": ["tB", "tC"] }));
        area.settle_tree(&e).await;
        assert_eq!(w.log_of("mark ").last().unwrap(), &format!("[\"tB\",\"tC\"] {T0}"));

        // kept for another branch, kept for none, or a pull request with no branch: nobody
        for (threads, pr) in [
            (Some(json!({ "branch": "fix/other", "threads": ["tA"] })), settled_pr()),
            (None, settled_pr()),
            (Some(json!({ "branch": "fix/ci", "threads": ["tA"] })), json!({ "state": "merged", "mergedAt": T0 - 1, "branch": null })),
        ] {
            let (w, area, _e) = setup();
            let e = w.folder();
            let mut v = rec_of(&e);
            v["pr"] = with(kept_pr(json!({})), pr.clone());
            if pr["branch"].is_null() {
                v["pr"].as_object_mut().unwrap().remove("branch");
            }
            if let Some(t) = threads {
                v["branchThreads"] = t;
            }
            *e.borrow().record.borrow_mut() = parsed(v);
            rows(&w);
            area.settle_tree(&e).await;
            assert!(w.log.borrow().is_empty());
        }

        // a worktree is its own branch: every root settles
        let (w, area, _e) = setup();
        let e = w.entry(json!({ "kind": "local", "machineId": "", "worktree": { "path": "/p", "branch": "fix/ci", "made": true } }));
        keep(&e, settled_pr());
        rows(&w);
        area.settle_tree(&e).await;
        assert_eq!(w.log_of("mark "), vec![format!("[\"tA\",\"tB\",\"tC\",\"tD\"] {T0}")]);
    });
}

// Q13: a mark that fails and a nap that fails each warn once, in the TS's words; the nap is tried after a failed mark.
#[test]
fn q13_a_failed_mark_or_nap_warns_whole_lines() {
    run(async {
        let (w, area, e) = setup();
        keep(&e, settled_pr());
        w.views.borrow_mut().push(sv("s1", "w1", Some("tA"), None, "completed"));
        w.pauses.set(true);
        *w.mark_error.borrow_mut() = Some("boom".into());
        *w.nap_error.borrow_mut() = Some("no machine".into());
        area.settle_tree(&e).await;
        settle().await;
        let warns = oracle("pull_requests")["warns"].clone();
        let got: Vec<String> = w.log_of("warn ");
        assert_eq!(json!(got), warns);
        assert_eq!(w.log_of("nap "), vec!["w1".to_string()]);
    });
}

// ---- Q14

// Q14: the repository settings of a remote, held an hour, read on this computer through the rate limit hold.
#[test]
fn q14_merge_settings_held_an_hour() {
    run(async {
        let (w, area, _e) = setup();
        w.here_says("git.repoRead", |_| ok(merge_json()));
        let frame_for = |remote: &str| json!({ "op": "git.repoRead", "cwd": HOME, "remote": remote }).to_string();
        let want = parsed::<GitRepoReadReply>(merge_json());
        assert_eq!(area.merge_settings(REMOTE).await.unwrap(), want);
        assert_eq!(w.frames("here"), vec![frame_for(REMOTE)]);
        assert_eq!(area.merge_settings(REMOTE).await.unwrap(), want);
        w.advance(SETTINGS_MS - 1);
        assert_eq!(area.merge_settings(REMOTE).await.unwrap(), want);
        assert_eq!(w.frames("here").len(), 1);
        w.advance(1);
        assert_eq!(area.merge_settings(REMOTE).await.unwrap(), want);
        assert_eq!(w.frames("here").len(), 2);
        // another remote is its own
        let other = "https://github.com/wsp/other.git";
        area.merge_settings(other).await.unwrap();
        assert_eq!(w.frames("here").last().unwrap(), &frame_for(other));
        area.merge_settings(REMOTE).await.unwrap();
        assert_eq!(w.frames("here").len(), 3);

        // a refusal, and a reply that does not parse, are not held
        let (w, area, _e) = setup();
        w.here_says("git.repoRead", |_| refused(None, "no"));
        let err = area.merge_settings(REMOTE).await.unwrap_err();
        assert_eq!(refusal(&err), (None, "no".to_string()));
        w.here_says("git.repoRead", |_| ok(json!({ "methods": "squash" })));
        assert!(area.merge_settings(REMOTE).await.is_err());
        w.here_says("git.repoRead", |_| ok(merge_json()));
        assert_eq!(area.merge_settings(REMOTE).await.unwrap(), want);
        assert_eq!(w.frames("here").len(), 3);

        // a rate limit holds the road: the next ask runs nothing for a minute
        let (w, area, _e) = setup();
        w.here_says("git.repoRead", |_| rate_limited());
        let err = area.merge_settings(REMOTE).await.unwrap_err();
        assert_eq!(refusal(&err), (Some("rate-limited".into()), RL_TEXT.into()));
        w.here_says("git.repoRead", |_| ok(merge_json()));
        let err = area.merge_settings(REMOTE).await.unwrap_err();
        assert_eq!(refusal(&err), (Some("rate-limited".into()), RL_TEXT.into()));
        assert_eq!(w.frames("here").len(), 1);
        w.advance(HOLD);
        assert_eq!(area.merge_settings(REMOTE).await.unwrap(), want);
        assert_eq!(w.frames("here").len(), 2);
    });
}

// Q15: git.update on the copy's checkout with the base, through the workspace's queue; then the checkout read, forced
// and awaited, and the pull request read, forced and not awaited.
#[test]
fn q15_update_copy_reads_after() {
    run(async {
        let (w, area, e) = setup();
        w.on_branch(&e, "fix/ci", "h1");
        e.borrow_mut().checkout.take();
        w.copy_says("git.update", |_| ok(update_json(false, 0, json!(["README.md"]))));
        // the pull request read waits on this computer; the update answers regardless
        w.here.hold.set(true);
        let got = area.update_copy(&e).await.unwrap();
        assert_eq!(got, parsed::<GitUpdateReply>(update_json(false, 0, json!(["README.md"]))));
        settle().await;
        let update = json!({ "op": "git.update", "cwd": COPY, "base": "main" }).to_string();
        assert_eq!(
            w.log.borrow().clone(),
            vec![
                "queued w1".to_string(),
                "road copy".to_string(),
                format!("frame copy {update}"),
                "checkout true".to_string(),
                "road here".to_string(),
                format!("frame here {}", pr_frame(HOME, json!({ "branch": "fix/ci" }))),
            ]
        );
        w.release("here");
        settle().await;
        assert_eq!(pr_of(&e)["number"], json!(12));

        // the record's base outranks the project's; none anywhere sends none
        let (w, area, e) = setup();
        set_record(&e, "base", json!("develop"));
        w.copy_says("git.update", |_| ok(update_json(true, 2, json!([]))));
        area.update_copy(&e).await.unwrap();
        assert_eq!(w.frames("copy"), vec![json!({ "op": "git.update", "cwd": COPY, "base": "develop" }).to_string()]);
        let (w, area, e) = setup();
        w.set_project(None, "main", REMOTE);
        w.copy_says("git.update", |_| ok(update_json(true, 2, json!([]))));
        area.update_copy(&e).await.unwrap();
        assert_eq!(w.frames("copy"), vec![json!({ "op": "git.update", "cwd": COPY }).to_string()]);

        // a failed update reads nothing after it
        let (w, area, e) = setup();
        w.copy_says("git.update", |_| refused(None, "git said: conflict"));
        let err = area.update_copy(&e).await.unwrap_err();
        assert_eq!(refusal(&err), (None, "git said: conflict".to_string()));
        settle().await;
        assert_eq!(w.log_of("checkout "), Vec::<String>::new());
        assert!(w.frames("here").is_empty());
    });
}

// ---- Q16

// Q16: the child's record keeps what it was given, or drops it when it is empty; then the lead's rows are read.
#[test]
fn q16_keep_tree_sets_or_removes_and_reads_the_lead() {
    run(async {
        let (w, area, child) = setup();
        let why = "no credential here for git.example.com; sign in and bring back again";
        area.keep_tree(&child, parsed::<TreeRecord>(json!({ "pushRefused": why }))).await.unwrap();
        assert_eq!(rec_of(&child)["tree"], json!({ "pushRefused": why }));
        assert_eq!(w.fields_persisted(), vec![format!("{{\"tree\":{}}}", json!({ "pushRefused": why }))]);
        assert_eq!(w.log.borrow().clone(), vec!["persist".to_string(), "lead w1".to_string()]);
        // empty removes it
        area.keep_tree(&child, parsed::<TreeRecord>(json!({}))).await.unwrap();
        assert!(rec_of(&child).get("tree").is_none());
        assert_eq!(w.fields_persisted().last().unwrap(), "{}");
        assert_eq!(w.log.borrow().len(), 4);
        // a merge and the files a merge stopped on
        let merged = json!({ "merged": { "oid": "d00d", "at": T0, "head": "c0de" } });
        area.keep_tree(&child, parsed::<TreeRecord>(merged.clone())).await.unwrap();
        assert_eq!(rec_of(&child)["tree"], merged);
        let stopped = json!({ "conflicts": ["lead.txt"] });
        area.keep_tree(&child, parsed::<TreeRecord>(stopped.clone())).await.unwrap();
        assert_eq!(rec_of(&child)["tree"], stopped);
        assert_eq!(w.persisted.borrow().len(), 4);
        assert_eq!(w.log_of("lead "), vec!["w1".to_string(); 4]);
    });
}

// ---- ported from the TS tests, at the area's seam

// TS: test/pull-request.test.ts:215
#[test]
fn ts_a_read_goes_over_this_computer_with_the_projects_remote_and_the_copys_branch() {
    run(async {
        let (w, area, e) = setup();
        *w.checkout_answer.borrow_mut() = Some(checkout_json("fix/ci", "abc"));
        let got = area.read_pull_request(&e, false, false).await;
        assert_eq!(pr_value(&got), fact(json!({}), T0));
        assert_eq!(
            w.log.borrow().clone(),
            vec![
                "checkout false".to_string(),
                "road here".to_string(),
                format!("frame here {}", json!({ "op": "git.prRead", "cwd": HOME, "remote": REMOTE, "branch": "fix/ci" })),
                "persist".to_string(),
                "status".to_string(),
            ]
        );
        assert_eq!(w.roads("copy"), 0);
        assert_eq!(pr_of(&e), fact(json!({}), T0));
        assert_eq!(w.fields_persisted(), vec![format!("{{\"pr\":{}}}", kept_pr(json!({})))]);
        w.assert_due(&[T0 + IDLE]);
    });
}

// TS: test/pull-request.test.ts:231
#[test]
fn ts_the_copy_is_asked_only_where_it_runs_and_a_stopped_one_is_never_woken() {
    run(async {
        let (w, area, e) = setup();
        w.on_branch(&e, "fix/ci", "h1");
        w.here_says("git.prRead", |_| no_gh());
        w.copy_says("git.prRead", |_| ok(json!({ "pr": open(json!({ "number": 3 })) })));
        let got = area.read_pull_request(&e, false, false).await;
        assert_eq!(pr_value(&got), fact(json!({ "number": 3 }), T0));
        assert_eq!(
            w.log.borrow().clone(),
            vec![
                "road here".to_string(),
                format!("frame here {}", pr_frame(HOME, json!({ "branch": "fix/ci" }))),
                "road copy".to_string(),
                format!("frame copy {}", pr_frame(COPY, json!({ "branch": "fix/ci" }))),
                "persist".to_string(),
                "status".to_string(),
            ]
        );
        // napped, a read after the poll interval says why it is not read, and the copy is not asked
        set_record(&e, "phase", json!("napping"));
        let armed = e.borrow_mut().pr_poll.take();
        armed.expect("the timer a read armed")();
        w.advance(IDLE);
        w.log.borrow_mut().clear();
        let got = area.read_pull_request(&e, false, false).await;
        assert_eq!(pr_value(&got), why(oracle("pull_requests")["stopped"].as_str().unwrap(), T0 + IDLE));
        assert_eq!(
            w.log.borrow().clone(),
            vec!["road here".to_string(), format!("frame here {}", pr_frame(HOME, json!({ "number": 3 }))), "status".to_string()]
        );
    });
}

// TS: test/pull-request.test.ts:251
#[test]
fn ts_neither_road_has_a_command_line_so_the_pull_request_is_not_read() {
    run(async {
        let (w, area, e) = setup();
        w.on_branch(&e, "fix/ci", "h1");
        w.here_says("git.prRead", |_| no_gh());
        w.copy_says("git.prRead", |_| no_gh());
        let got = area.read_pull_request(&e, false, false).await;
        assert_eq!(pr_value(&got), why(oracle("pull_requests")["unread"].as_str().unwrap(), T0));
        assert_eq!(w.statuses.borrow().clone(), vec![why(oracle("pull_requests")["unread"].as_str().unwrap(), T0)]);
    });
}

// TS: test/pull-request.test.ts:259
#[test]
fn ts_a_failed_read_keeps_the_fact_and_reads_again_on_its_timer() {
    run(async {
        let (w, area, e) = setup();
        w.on_branch(&e, "fix/ci", "h1");
        answer_pr(&w, open(json!({ "checks": [{ "name": "ci", "state": "pass" }] })));
        area.read_pull_request(&e, false, false).await;
        let held = fact(json!({ "checks": [{ "name": "ci", "state": "pass" }] }), T0);
        assert_eq!(pr_of(&e), held);
        w.here_says("git.prRead", |_| refused(None, GH_DOWN));
        w.advance(IDLE);
        settle().await;
        assert_eq!(w.frames("here").len(), 2);
        assert_eq!(w.frames("here")[1], pr_frame(HOME, json!({ "number": 12 })));
        assert_eq!(pr_of(&e), held);
        assert_eq!(w.statuses.borrow().len(), 1);
        assert_eq!(w.persisted.borrow().len(), 1);
        w.assert_due(&[T0 + 2 * IDLE]);
        answer_pr(&w, open(json!({ "state": "merged" })));
        w.advance(IDLE);
        settle().await;
        assert_eq!(w.frames("here").len(), 3);
        assert_eq!(pr_of(&e), fact(json!({ "state": "merged" }), T0 + 2 * IDLE));
        assert_eq!(w.persisted.borrow().len(), 2);
        w.assert_due(&[]);
    });
}

// TS: test/pull-request.test.ts:277
#[test]
fn ts_merged_and_closed_are_recorded_as_the_host_says_them() {
    run(async {
        for (state, at_key) in [("merged", "mergedAt"), ("closed", "closedAt")] {
            let (w, area, e) = setup();
            w.on_branch(&e, "fix/ci", "h1");
            answer_pr(&w, open(json!({ "state": state })));
            area.read_pull_request(&e, false, false).await;
            assert_eq!(pr_of(&e), fact(json!({ "state": state }), T0));
            let mut kept = kept_pr(json!({ "state": state }));
            kept[at_key] = json!(T0);
            kept["checks"] = open(json!({}))["checks"].clone();
            assert_eq!(record_pr(&e), kept.to_string(), "{state}");
            w.assert_due(&[]);
        }
    });
}

// TS: test/pull-request.test.ts:285
#[test]
fn ts_an_open_one_is_read_on_its_timer_whatever_its_checks_say_and_never_again_once_merged() {
    run(async {
        let (w, area, e) = setup();
        w.on_branch(&e, "fix/ci", "h1");
        answer_pr(&w, open(json!({ "checks": [] })));
        area.read_pull_request(&e, false, false).await;
        assert_eq!(w.frames("here").len(), 1);
        answer_pr(&w, open(json!({ "checks": [{ "name": "ci", "state": "pass" }] })));
        w.advance(IDLE);
        settle().await;
        assert_eq!(w.frames("here").len(), 2);
        answer_pr(&w, open(json!({ "state": "merged", "checks": [{ "name": "ci", "state": "pass" }] })));
        w.advance(IDLE);
        settle().await;
        assert_eq!(w.frames("here").len(), 3);
        assert_eq!(pr_of(&e)["state"], json!("merged"));
        assert_eq!(w.persisted.borrow().len(), 2);
        // never again, whatever asks
        w.advance(3 * IDLE);
        w.advance(20_000);
        settle().await;
        area.read_pull_request(&e, false, false).await;
        area.read_pull_request(&e, true, true).await;
        assert_eq!(w.frames("here").len(), 3);
        w.assert_due(&[]);
    });
}

// TS: test/pull-request.test.ts:317
#[test]
fn ts_a_read_asks_with_what_the_last_read_saw_and_an_unchanged_answer_keeps_the_fact() {
    run(async {
        let (w, area, e) = setup();
        w.on_branch(&e, "fix/ci", "h1");
        w.here_says("git.prRead", |_| ok(json!({ "pr": open(json!({})), "seen": SEEN })));
        area.read_pull_request(&e, false, false).await;
        let key = PullRequestsArea::pr_key(REMOTE, 12);
        let held = w.cached(&key).unwrap();
        assert_eq!((held.tag.clone(), held.fetched_at), (Some(SEEN.to_string()), T0));
        w.here_says("git.prRead", |_| ok(json!({ "unchanged": true, "seen": SEEN })));
        w.advance(IDLE);
        settle().await;
        assert_eq!(w.frames("here")[1], pr_frame(HOME, json!({ "number": 12, "seen": SEEN })));
        assert_eq!(pr_of(&e), fact(json!({}), T0 + IDLE));
        assert_eq!(w.persisted.borrow().len(), 1);
        // a changed one answers in full with what it saw now, which the next read carries
        w.here_says("git.prRead", |_| ok(json!({ "pr": open(json!({ "headOid": "c0ffee" })), "seen": MOVED })));
        w.advance(IDLE);
        settle().await;
        assert_eq!(w.frames("here")[2], pr_frame(HOME, json!({ "number": 12, "seen": SEEN })));
        assert_eq!(pr_of(&e), fact(json!({ "headOid": "c0ffee" }), T0 + 2 * IDLE));
        assert_eq!(w.cached(&key).unwrap().tag, Some(MOVED.to_string()));
        w.advance(IDLE);
        settle().await;
        assert_eq!(w.frames("here")[3], pr_frame(HOME, json!({ "number": 12, "seen": MOVED })));
    });
}

// TS: test/pull-request.test.ts:340
#[test]
fn ts_a_check_still_running_is_asked_for_in_full() {
    run(async {
        let (w, area, e) = setup();
        w.on_branch(&e, "fix/ci", "h1");
        w.here_says("git.prRead", |_| ok(json!({ "pr": open(json!({ "checks": [{ "name": "ci", "state": "pending" }] })), "seen": SEEN })));
        area.read_pull_request(&e, false, false).await;
        w.advance(IDLE);
        settle().await;
        assert_eq!(w.frames("here").len(), 2);
        assert_eq!(w.frames("here")[1], pr_frame(HOME, json!({ "number": 12 })));
    });
}

// TS: test/pull-request.test.ts:351
#[test]
fn ts_an_open_one_is_read_every_poll_ms_with_a_window_and_every_idle_ms_with_none() {
    run(async {
        let (w, area, e) = setup();
        w.on_branch(&e, "fix/ci", "h1");
        area.read_pull_request(&e, false, false).await;
        assert_eq!(w.frames("here").len(), 1);
        w.advance(POLL);
        settle().await;
        assert_eq!(w.frames("here").len(), 1);
        w.advance(IDLE - POLL);
        settle().await;
        assert_eq!(w.frames("here").len(), 2);
        // a window opens just after that read: the sidebar's ask reads nothing, and the timer moves to three minutes
        w.watched.set(true);
        area.read_pull_request(&e, false, false).await;
        settle().await;
        assert_eq!(w.frames("here").len(), 2);
        w.advance(POLL);
        settle().await;
        assert_eq!(w.frames("here").len(), 3);
        w.advance(POLL);
        settle().await;
        assert_eq!(w.frames("here").len(), 4);
        w.advance(CHECKOUT_TTL + 1);
        area.read_pull_request(&e, false, false).await;
        settle().await;
        assert_eq!(w.frames("here").len(), 4);
    });
}

// TS: test/pull-request.test.ts:386
#[test]
fn ts_a_branch_with_no_pull_request_is_read_again_on_view_only_once_its_head_moves() {
    run(async {
        let (w, area, e) = setup();
        answer_none(&w);
        w.on_branch(&e, "fix/ci", "abc1234");
        area.read_pull_request(&e, false, false).await;
        assert_eq!(w.frames("here").len(), 1);
        for _ in 0..3 {
            w.advance(CHECKOUT_TTL + 1);
            area.read_pull_request(&e, false, false).await;
        }
        assert_eq!(w.frames("here").len(), 1);
        w.advance(CHECKOUT_TTL + 1);
        w.on_branch(&e, "fix/ci", "def0123");
        area.read_pull_request(&e, false, false).await;
        assert_eq!(w.frames("here").len(), 2);
    });
}

// TS: test/pull-request.test.ts:412
#[test]
fn ts_a_rate_limit_holds_pages_and_status_reads_on_that_road_for_a_minute() {
    run(async {
        let (w, area, e) = setup();
        w.on_branch(&e, "fix/ci", "h1");
        w.here_says("git.prView", |_| rate_limited());
        area.read_pull_request(&e, false, false).await;
        let reads = w.frames("here").len();
        w.advance(IDLE - 30_000);
        let err = area.read_page(&e, REMOTE, 12, false).await.map(|_| ()).unwrap_err();
        assert_eq!(refusal(&err), (Some("rate-limited".into()), RL_TEXT.into()));
        for _ in 0..3 {
            let err = area.read_page(&e, REMOTE, 12, true).await.map(|_| ()).unwrap_err();
            assert_eq!(refusal(&err), (Some("rate-limited".into()), RL_TEXT.into()));
        }
        let views = |w: &World| w.frames("here").iter().filter(|f| f.contains("git.prView")).count();
        assert_eq!(views(&w), 1);
        // the status timer falling inside it asks no more, and the fact it held stands
        w.advance(30_000);
        settle().await;
        assert_eq!(w.frames("here").len(), reads + 1);
        assert_eq!(pr_of(&e), fact(json!({}), T0));
        w.here_says("git.prView", |_| ok(page_json()));
        w.advance(HOLD - 30_000);
        area.read_page(&e, REMOTE, 12, false).await.unwrap();
        assert_eq!(views(&w), 2);
    });
}

// TS: test/pull-request.test.ts:439
#[test]
fn ts_a_merged_pull_request_stamps_a_quiet_tree_and_naps_its_machine() {
    run(async {
        let (w, area, e) = setup();
        w.on_branch(&e, "fix/ci", "h1");
        w.pauses.set(true);
        w.views.borrow_mut().push(sv("s1", "w1", Some("tL"), None, "completed"));
        area.read_pull_request(&e, false, false).await;
        settle().await;
        assert!(w.log_of("mark ").is_empty());
        answer_pr(&w, open(json!({ "state": "merged" })));
        w.advance(IDLE);
        settle().await;
        assert_eq!(w.log_of("mark "), vec![format!("[\"tL\"] {}", T0 + IDLE)]);
        assert_eq!(w.log_of("nap "), vec!["w1".to_string()]);
        let marked = w.log.borrow().iter().position(|l| l.starts_with("mark ")).unwrap();
        let napped = w.log.borrow().iter().position(|l| l.starts_with("nap ")).unwrap();
        assert!(marked < napped);
        // restored, nothing is read again: a merged one is never read
        let reads = w.frames("here").len();
        w.advance(CHECKOUT_TTL + 1);
        area.read_pull_request(&e, false, false).await;
        settle().await;
        assert_eq!(w.frames("here").len(), reads);
    });
}

// TS: test/pull-request.test.ts:467
#[test]
fn ts_a_project_folder_keeps_the_pull_request_with_its_branch_and_loses_it_when_it_leaves() {
    run(async {
        let (w, area, _e) = setup();
        let e = w.folder();
        w.here_says("git.prRead", |f| {
            if f.get("branch") == Some(&json!("fix/ci")) || f.get("number") == Some(&json!(12)) {
                ok(json!({ "pr": open(json!({ "state": "merged" })) }))
            } else {
                ok(json!({}))
            }
        });
        for t in ["tC", "tA", "tB"] {
            w.views.borrow_mut().push(sv(&format!("s{t}"), "w1", Some(t), None, "completed"));
        }
        // C ran on main before the folder went to the branch: nothing is kept for it
        w.on_branch(&e, "main", "h0");
        area.ran_on_branch(&e, "tC").await.unwrap();
        assert!(w.persisted.borrow().is_empty());
        // A's turn ended on the branch
        w.on_branch(&e, "fix/ci", "h1");
        area.ran_on_branch(&e, "tA").await.unwrap();
        area.read_pull_request(&e, true, false).await;
        settle().await;
        assert_eq!(w.log_of("mark "), vec![format!("[\"tA\"] {T0}")]);
        let kept = json!({ "number": 12, "url": "https://github.com/wsp/pr-lab/pull/12", "state": "merged", "base": "main", "branch": "fix/ci", "mergedAt": T0, "checks": open(json!({}))["checks"].clone() });
        assert_eq!(rec_of(&e)["pr"], kept);
        assert_eq!(rec_of(&e)["branchThreads"], json!({ "branch": "fix/ci", "threads": ["tA"] }));
        // B ends back on main: the folder keeps none of the base's threads, the merged pull request is not its any more
        w.on_branch(&e, "main", "h2");
        area.ran_on_branch(&e, "tB").await.unwrap();
        assert!(rec_of(&e).get("branchThreads").is_none());
        let got = area.read_pull_request(&e, false, false).await;
        assert_eq!(pr_value(&got), Value::Null);
        settle().await;
        assert!(rec_of(&e).get("pr").is_none());
        assert_eq!(w.log_of("mark ").len(), 1);
        assert_eq!(w.fields_persisted().last().unwrap(), "{}");
    });
}

// TS: test/pull-request.test.ts:513, test/worktree-sweep.test.ts:329
#[test]
fn ts_a_fork_pull_request_on_a_worktree_is_read_by_number_alone_and_kept() {
    run(async {
        let (w, area, _e) = setup();
        let e = w.entry(json!({ "kind": "local", "machineId": "", "worktree": { "path": "/home/u/.wsp/trees/feat", "branch": "ana/feat", "made": true } }));
        keep(&e, json!({ "branch": "feat" }));
        w.on_branch(&e, "ana/feat", "h1");
        w.here_says("git.prRead", |f| {
            if f.get("number") == Some(&json!(12)) {
                ok(json!({ "pr": open(json!({ "branch": "feat", "fork": { "owner": "ana", "pushable": true } })) }))
            } else {
                ok(json!({}))
            }
        });
        for _ in 0..3 {
            area.read_pull_request(&e, true, false).await;
        }
        assert!(!area.left_branch(&e));
        assert_eq!(w.frames("here"), vec![pr_frame(HOME, json!({ "number": 12 })); 3]);
        assert_eq!(rec_of(&e)["pr"]["number"], json!(12));
        assert!(w.persisted.borrow().is_empty());
    });
}

// TS: test/pull-request.test.ts:544
#[test]
fn ts_a_merged_tree_stays_live_while_a_child_works_and_is_stamped_when_it_ends() {
    run(async {
        let (w, area, e) = setup();
        w.pauses.set(true);
        w.views.borrow_mut().push(sv("s1", "w1", Some("tL"), None, "completed"));
        w.views.borrow_mut().push(sv("s2", "w1", Some("tC"), Some("tL"), "running"));
        area.take_pull_request(&e, Some(seen_of(fact(json!({ "state": "merged" }), T0)))).await.unwrap();
        settle().await;
        assert!(w.log_of("mark ").is_empty());
        assert!(w.log_of("nap ").is_empty());
        w.views.borrow_mut()[1] = sv("s2", "w1", Some("tC"), Some("tL"), "completed");
        area.settle_tree(&e).await;
        settle().await;
        assert_eq!(w.log_of("mark "), vec![format!("[\"tL\",\"tC\"] {T0}")]);
        assert_eq!(w.log_of("nap "), vec!["w1".to_string()]);
    });
}

// TS: test/pull-request.test.ts:564
#[test]
fn ts_a_workspace_a_thread_opened_settles_nothing_off_its_own_pull_request() {
    run(async {
        let (w, area, _e) = setup();
        let child = w.entry(json!({ "id": "w2", "parentThreadId": "tL", "rootThreadId": "tL" }));
        w.pauses.set(true);
        w.views.borrow_mut().push(sv("s1", "w2", Some("tK"), None, "completed"));
        area.take_pull_request(&child, Some(seen_of(fact(json!({ "state": "merged" }), T0)))).await.unwrap();
        area.settle_tree(&child).await;
        settle().await;
        assert!(w.log_of("mark ").is_empty());
        assert!(w.log_of("nap ").is_empty());
        assert_eq!(rec_of(&child)["pr"]["state"], json!("merged"));
    });
}

// TS: test/pull-request.test.ts:610
#[test]
fn ts_a_page_write_and_the_settings_cost_one_read_each_within_their_holds() {
    run(async {
        let (w, area, e) = setup();
        w.here_says("git.prView", |_| ok(page_json()));
        w.here_says("git.repoRead", |_| ok(merge_json()));
        w.here_says("git.prReact", |f| ok(json!({ "subject": f["subject"].clone(), "reactions": [] })));
        area.merge_settings(REMOTE).await.unwrap();
        area.read_page(&e, REMOTE, 12, false).await.unwrap();
        assert_eq!(w.frames("here"), vec![json!({ "op": "git.repoRead", "cwd": HOME, "remote": REMOTE }).to_string(), page_frame(HOME)]);
        w.advance(PAGE_HOLD - 1);
        area.read_page(&e, REMOTE, 12, false).await.unwrap();
        assert_eq!(w.frames("here").len(), 2);
        w.advance(1);
        area.read_page(&e, REMOTE, 12, false).await.unwrap();
        area.read_page(&e, REMOTE, 12, true).await.unwrap();
        assert_eq!(w.frames("here").len(), 4);
        area.post_as_person(REMOTE, 12, writes().pop().unwrap().1, whole()).await.unwrap();
        area.read_page(&e, REMOTE, 12, false).await.unwrap();
        area.merge_settings(REMOTE).await.unwrap();
        let frames = w.frames("here");
        assert_eq!(frames.iter().filter(|f| f.contains("git.prView")).count(), 4);
        assert_eq!(frames.iter().filter(|f| f.contains("git.repoRead")).count(), 1);
    });
}

// TS: test/pull-request.test.ts:751
#[test]
fn ts_a_reply_a_resolve_and_a_reaction_post_as_the_person_over_this_computers_daemon() {
    run(async {
        let (w, area, _e) = setup();
        let comment = json!({ "id": 9, "nodeId": "IC_kwDOx", "author": "Zingzy", "association": "owner", "bot": false, "body": "Fixed.", "url": "u", "at": "t" });
        let c = comment.clone();
        w.here_says("git.prReply", move |_| ok(json!({ "comment": c.clone() })));
        w.here_says("git.prResolve", |f| ok(json!({ "threadId": f["threadId"].clone(), "resolved": f["resolved"].clone() })));
        w.here_says("git.prReact", |f| {
            ok(json!({ "subject": f["subject"].clone(), "reactions": [{ "content": "eyes", "count": 1, "mine": true }] }))
        });
        let mut got = vec![];
        for (_, frame_for) in writes() {
            got.push(Value::Object(area.post_as_person(REMOTE, 12, frame_for, whole()).await.unwrap()));
        }
        assert_eq!(got[0]["comment"], comment);
        assert_eq!((got[1]["threadId"].clone(), got[1]["resolved"].clone()), (json!("PRRT_kwDOx"), json!(true)));
        assert_eq!(got[2]["reactions"], json!([{ "content": "eyes", "count": 1, "mine": true }]));
        assert_eq!(w.frames("here"), vec![write_frame("git.prReply"), write_frame("git.prResolve"), write_frame("git.prReact")]);
        assert_eq!(w.roads("copy"), 0);
    });
}

// TS: test/pull-request.test.ts:804
#[test]
fn ts_a_settled_record_keeps_the_checks_the_read_that_saw_it_settle_found() {
    run(async {
        let (w, area, e) = setup();
        w.on_branch(&e, "fix/ci", "h1");
        area.read_pull_request(&e, false, false).await;
        assert!(rec_of(&e)["pr"].get("checks").is_none());
        let checks = json!([{ "name": "ci", "workflow": "ci", "state": "pass", "startedAt": "2026-10-02T10:00:00Z", "completedAt": "2026-10-02T10:16:00Z" }]);
        answer_pr(&w, open(json!({ "state": "merged", "checks": checks.clone() })));
        w.advance(IDLE);
        settle().await;
        let merged = json!({ "number": 12, "url": "https://github.com/wsp/pr-lab/pull/12", "state": "merged", "base": "main", "branch": "fix/ci", "mergedAt": T0 + IDLE, "checks": checks });
        assert_eq!(rec_of(&e)["pr"], merged);
        assert_eq!(record_pr(&e), merged.to_string());
    });
}

// TS: test/pull-request.test.ts:846
#[test]
fn ts_a_whole_read_never_asks_with_what_the_last_read_saw() {
    run(async {
        let (w, area, e) = setup();
        w.on_branch(&e, "fix/ci", "h1");
        let checks =
            Rc::new(RefCell::new(json!([{ "name": "ci", "workflow": "ci", "state": "pass", "run": { "runId": 36, "jobId": 109 } }])));
        let c = checks.clone();
        w.here_says("git.prRead", move |f| {
            if f.get("seen") == Some(&json!(SEEN)) {
                ok(json!({ "unchanged": true, "seen": SEEN }))
            } else {
                ok(json!({ "pr": open(json!({ "checks": c.borrow().clone() })), "seen": SEEN }))
            }
        });
        area.read_pull_request(&e, false, false).await;
        *checks.borrow_mut() = json!([{ "name": "ci", "workflow": "ci", "state": "fail", "run": { "runId": 36, "jobId": 109 } }]);
        w.advance(5_000);
        // a forced read asks with what it saw and is told nothing moved
        area.read_pull_request(&e, true, false).await;
        assert_eq!(w.frames("here")[1], pr_frame(HOME, json!({ "number": 12, "seen": SEEN })));
        assert_eq!(pr_of(&e)["checks"][0]["state"], json!("pass"));
        // a whole one does not
        let got = area.read_pull_request(&e, true, true).await;
        assert_eq!(w.frames("here")[2], pr_frame(HOME, json!({ "number": 12 })));
        assert_eq!(pr_value(&got)["checks"][0]["state"], json!("fail"));
        assert_eq!(pr_of(&e)["readAt"], json!(T0 + 5_000));
    });
}

// TS: test/pull-request.test.ts:867
#[test]
fn ts_update_answers_a_clean_merge_and_the_conflicts_through_the_copys_daemon() {
    run(async {
        let (w, area, e) = setup();
        w.on_branch(&e, "fix/ci", "h1");
        let answer = Rc::new(RefCell::new(update_json(true, 2, json!([]))));
        let a = answer.clone();
        w.copy_says("git.update", move |_| ok(a.borrow().clone()));
        assert_eq!(area.update_copy(&e).await.unwrap(), parsed::<GitUpdateReply>(update_json(true, 2, json!([]))));
        *answer.borrow_mut() = update_json(false, 0, json!(["README.md"]));
        assert_eq!(area.update_copy(&e).await.unwrap(), parsed::<GitUpdateReply>(update_json(false, 0, json!(["README.md"]))));
        let update = json!({ "op": "git.update", "cwd": COPY, "base": "main" }).to_string();
        assert_eq!(w.frames("copy"), vec![update.clone(), update]);
        assert_eq!(w.log_of("queued "), vec!["w1".to_string(), "w1".to_string()]);
    });
}

// TS: test/tree.test.ts:351, :418, :432
#[test]
fn ts_the_tree_a_child_keeps_is_what_its_lead_reads() {
    run(async {
        let (w, area, child) = setup();
        let refused = "no credential here for git.example.com; sign in and bring back again";
        area.keep_tree(&child, parsed(json!({ "pushRefused": refused }))).await.unwrap();
        assert_eq!(rec_of(&child)["tree"]["pushRefused"], json!(refused));
        area.keep_tree(&child, parsed(json!({}))).await.unwrap();
        assert!(rec_of(&child).get("tree").is_none());
        area.keep_tree(&child, parsed(json!({ "conflicts": ["lead.txt"] }))).await.unwrap();
        assert_eq!(rec_of(&child)["tree"], json!({ "conflicts": ["lead.txt"] }));
        area.keep_tree(&child, parsed(json!({ "merged": { "oid": "d00d", "at": T0 } }))).await.unwrap();
        assert_eq!(rec_of(&child)["tree"], json!({ "merged": { "oid": "d00d", "at": T0 } }));
        assert!(rec_of(&child)["tree"].get("conflicts").is_none());
        area.keep_tree(&child, parsed(json!({ "merged": { "oid": "d00d", "at": T0, "head": "c0de" } }))).await.unwrap();
        assert_eq!(rec_of(&child)["tree"]["merged"]["head"], json!("c0de"));
        assert_eq!(w.log_of("lead ").len(), 5);
    });
}
