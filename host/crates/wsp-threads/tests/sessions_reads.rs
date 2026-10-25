// SPDX-License-Identifier: AGPL-3.0-only
//! spec/sessions_reads.md. TS cases: thread-cap.test.ts, session-title.test.ts, runtime-history.test.ts,
//! thread-head.test.ts, session-images.test.ts, session-search.test.ts (the whole-runtime cases, ported at the seam).
mod support;

use std::cell::RefCell;
use std::rc::Rc;

use futures::channel::oneshot;
use support::*;
use wsp_core::types::internal::{SearchHit, ATTACHMENTS, ATTACHMENT_KEYS};
use wsp_wire::views::preferences::{HEAD_BYTES, HISTORY_PAGE_BYTES, HISTORY_PAGE_EVENTS};
use wsp_words::exit::{not_found_refusal, usage_refusal};
use wsp_words::words::computer::thread_word;

fn host() -> (Rc<FakeHost>, SessionsArea) {
    let host = FakeHost::new();
    host.add_entry(entry("w1", json!({})));
    let area = SessionsArea::new(host.clone());
    (host, area)
}

fn refusal(message: &str, kind: Option<Kind>) -> HostError {
    match kind {
        Some(k) => HostError::with_kind(message, k),
        None => HostError::new(message),
    }
}

fn put_row(host: &FakeHost, key: &str, workspace_id: &str, extra: Value) {
    host.add_row(key, row(view(key, workspace_id, extra), key));
}

/// The logged calls of the named methods, in order.
fn calls(host: &FakeHost, names: &[&str]) -> Vec<String> {
    host.hooks.calls().into_iter().filter(|c| names.iter().any(|n| c == n || c.starts_with(&format!("{n}(")))).collect()
}

fn json_of<T: serde::Serialize>(v: &T) -> Value {
    serde_json::to_value(v).expect("a value that serializes")
}

fn ev(kind: &str, thread_id: Option<&str>, extra: Value) -> SessionEvent {
    let mut doc = json!({ "type": kind, "workspaceId": "w1", "sessionId": "s1", "turnId": "turn-1" });
    if let Some(t) = thread_id {
        doc["threadId"] = json!(t);
    }
    for (k, v) in extra.as_object().expect("extra is an object") {
        doc[k] = v.clone();
    }
    event(doc)
}

fn end_ev(thread_id: &str) -> SessionEvent {
    ev("session.end", Some(thread_id), json!({ "sawResult": true }))
}

fn facts(id: &str, workspace_id: &str, extra: Value) -> ThreadFacts {
    let mut doc = json!({
        "id": id, "workspaceId": workspace_id, "harness": "claude", "startedBy": "person", "status": "completed",
        "title": "busy", "sessionId": id, "turns": 2, "ran": true,
    });
    for (k, v) in extra.as_object().expect("extra is an object") {
        doc[k] = v.clone();
    }
    serde_json::from_value(doc).expect("thread facts the fixture builds")
}

fn scoped(thread: &str) -> Caller {
    let by: ThreadScope = serde_json::from_value(json!({ "kind": "thread", "threadId": thread, "workspaceId": "w1", "rootThreadId": thread })).expect("a thread scope");
    Caller::Scoped { origin: WorkspaceOrigin::Relayed, by }
}

type TreeStandsOn = dyn Fn(&str, Option<&Caller>, Option<TreeTalk>) -> bool;
type ReachesRow = dyn Fn(&SessionView, Option<&Caller>, Option<TreeTalk>) -> bool;
type DrivesThread = dyn Fn(Option<&str>, Option<&Caller>) -> bool;
type OpenTranscript = dyn Fn(&str) -> LocalBoxFuture<'static, Result<Vec<SessionEvent>, HostError>>;
type OpenThread = dyn Fn(&str, &str) -> LocalBoxFuture<'static, Result<Vec<SessionEvent>, HostError>>;
type ReadPage = dyn Fn(&str, &str, ReadWindow) -> LocalBoxFuture<'static, Result<HistoryPage, HostError>>;
type EntryOfWith = dyn Fn(&str, Option<&Caller>, bool, bool) -> LocalBoxFuture<'static, Result<Entry, HostError>>;
type EntryOfRow = dyn Fn(Option<&str>, &str, Option<&Caller>, Option<TreeTalk>, bool) -> LocalBoxFuture<'static, Result<Option<Entry>, HostError>>;

fn drives_only(threads: &'static [&'static str]) -> Rc<DrivesThread> {
    Rc::new(move |t: Option<&str>, _o: Option<&Caller>| t.map(|t| threads.contains(&t)).unwrap_or(false))
}

// Q1, Q3: thread-cap.test.ts:238
#[test]
fn a_listing_leaves_out_rows_the_caller_cannot_reach() {
    run_local(async {
        let (host, area) = host();
        let capped = json!({ "placeId": "here", "place": "hetzner", "running": 2, "atOnce": 2 });
        put_row(&host, "r1", "w1", json!({ "threadId": "t1", "status": "completed", "capped": capped, "lastLine": "one" }));
        put_row(&host, "r2", "w1", json!({ "threadId": "t2", "status": "completed", "lastLine": "two", "failure": "broke" }));
        put_row(&host, "r3", "w1", json!({ "threadId": "t3", "status": "completed" }));
        put_row(&host, "r4", "w2", json!({ "threadId": "t4", "status": "completed" }));
        // For listing the caller reaches every thread but t3; in full it reaches every thread but t2.
        host.hooks.set::<ReachesRow>(
            "SessionsCtx::reaches_row",
            Rc::new(|v: &SessionView, _o: Option<&Caller>, talk: Option<TreeTalk>| match talk {
                Some(TreeTalk::List) => v.thread_id.as_deref() != Some("t3"),
                _ => v.thread_id.as_deref() != Some("t2"),
            }),
        );
        let origin = Some(scoped("t1"));

        let all = area.list_rows(None, origin.clone()).await.expect("a listing");
        let r1 = json!({ "id": "r1", "workspaceId": "w1", "harness": "claude", "status": "completed", "threadId": "t1", "capped": capped, "lastLine": "one" });
        let r2_named_only = json!({ "id": "r2", "workspaceId": "w1", "harness": "claude", "status": "completed", "threadId": "t2" });
        let r4 = json!({ "id": "r4", "workspaceId": "w2", "harness": "claude", "status": "completed", "threadId": "t4" });
        assert_eq!(json_of(&all), json!([r1, r2_named_only, r4]));

        let named = area.list_rows(Some("w1".into()), origin).await.expect("a listing of one workspace");
        assert_eq!(json_of(&named), json!([r1, r2_named_only]));
        assert_eq!(calls(&host, &["refuse_named", "tree_records"]), vec!["tree_records(None)".to_string(), "tree_records(Some(\"w1\"))".to_string()]);
    });
}

// Q1
#[test]
fn a_named_workspace_no_thread_of_the_tree_stands_on_is_refused() {
    run_local(async {
        let (host, area) = host();
        put_row(&host, "r1", "w1", json!({ "threadId": "t1", "status": "completed" }));
        let asked: Rc<RefCell<Vec<(String, Option<TreeTalk>)>>> = Rc::default();
        let seen = asked.clone();
        host.hooks.set::<TreeStandsOn>(
            "SessionsCtx::tree_stands_on",
            Rc::new(move |w: &str, _o: Option<&Caller>, talk: Option<TreeTalk>| {
                seen.borrow_mut().push((w.to_string(), talk));
                w == "w1"
            }),
        );
        host.hooks.set::<dyn Fn(&str, Option<&Caller>) -> Result<(), HostError>>(
            "SessionsCtx::refuse_named",
            Rc::new(|w: &str, _o: Option<&Caller>| Err(refusal(&format!("no such workspace {w}"), Some(Kind::NotFound)))),
        );
        let err = area.list_rows(Some("w2".into()), Some(scoped("t1"))).await.expect_err("a refusal");
        assert_eq!(said(&err), ("no such workspace w2".to_string(), None, Some(Kind::NotFound)));
        assert_eq!(*asked.borrow(), vec![("w2".to_string(), Some(TreeTalk::List))]);
        assert_eq!(calls(&host, &["listed_rows", "tree_records", "placed_rows"]), Vec::<String>::new(), "nothing is listed once refused");

        let ok = area.list_rows(Some("w1".into()), Some(scoped("t1"))).await.expect("a workspace a thread of the tree stands on");
        assert_eq!(ok.len(), 1);
        assert_eq!(calls(&host, &["refuse_named"]), vec!["refuse_named".to_string()], "only the workspace nothing stands on is refused");
    });
}

// Q3: thread-keeps-agent.test.ts:108, thread-lines.test.ts:170
#[test]
fn a_thread_whose_rows_fell_off_is_listed_off_its_record() {
    run_local(async {
        let (host, area) = host();
        put_row(&host, "r1", "w1", json!({ "threadId": "t1", "status": "completed" }));
        let held = |doc: Value| HeldThread { workspace_id: "w1".into(), record: thread_record(doc) };
        let records = vec![
            ("t1".to_string(), held(json!({ "harness": "claude" }))),
            ("t8".to_string(), held(json!({ "harness": "codex", "permissionMode": "read-only", "parentThreadId": "t1", "rootThreadId": "t1", "ended": "failed" }))),
            ("t9".to_string(), held(json!({ "harness": "claude" }))),
            ("t7".to_string(), held(json!({ "harness": "claude" }))),
        ];
        let talks: Rc<RefCell<Vec<TreeTalk>>> = Rc::default();
        let seen = talks.clone();
        host.hooks.set::<dyn Fn(Option<&Caller>, TreeTalk, Option<&str>) -> Vec<(String, HeldThread)>>(
            "SessionsCtx::tree_records",
            Rc::new(move |_o: Option<&Caller>, talk: TreeTalk, _w: Option<&str>| {
                seen.borrow_mut().push(talk);
                records.clone()
            }),
        );
        // t9's record holds no end, so the transcript's result says how it ended; t7 has neither.
        host.hooks.set::<OpenThread>(
            "SessionsCtx::open_thread",
            Rc::new(|_w: &str, t: &str| {
                let events = if t == "t9" { vec![ev("session.done", Some("t9"), json!({ "result": { "status": "interrupted" } }))] } else { Vec::new() };
                async move { Ok(events) }.boxed_local()
            }),
        );
        let listed_in: Rc<RefCell<Vec<Value>>> = Rc::default();
        let seen = listed_in.clone();
        host.hooks.set::<dyn Fn(Vec<SessionView>) -> Vec<SessionView>>(
            "SessionsCtx::listed_rows",
            Rc::new(move |rows: Vec<SessionView>| {
                seen.borrow_mut().push(json_of(&rows));
                rows
            }),
        );
        let placed_in: Rc<RefCell<Vec<Value>>> = Rc::default();
        let seen = placed_in.clone();
        host.hooks.set::<dyn Fn(Vec<SessionView>) -> Vec<SessionView>>(
            "SessionsCtx::placed_rows",
            Rc::new(move |mut rows: Vec<SessionView>| {
                seen.borrow_mut().push(json_of(&rows));
                rows.reverse();
                rows
            }),
        );

        let out = area.list_rows(Some("w1".into()), None).await.expect("a listing");
        let r1 = json!({ "id": "r1", "workspaceId": "w1", "harness": "claude", "status": "completed", "threadId": "t1" });
        let t8 = json!({ "id": "t8", "threadId": "t8", "workspaceId": "w1", "harness": "codex", "status": "failed", "startedBy": "agent", "parentThreadId": "t1", "rootThreadId": "t1", "permissionMode": "read-only" });
        let t9 = json!({ "id": "t9", "threadId": "t9", "workspaceId": "w1", "harness": "claude", "status": "interrupted" });
        let t7 = json!({ "id": "t7", "threadId": "t7", "workspaceId": "w1", "harness": "claude", "status": "completed" });
        assert_eq!(*talks.borrow(), vec![TreeTalk::List]);
        assert_eq!(*listed_in.borrow(), vec![json!([r1])], "the rows go through listed_rows, the trimmed rows do not");
        assert_eq!(*placed_in.borrow(), vec![json!([r1, t8, t9, t7])], "rows first, then the trimmed rows in record order, a thread with a row left out");
        assert_eq!(json_of(&out), json!([t7, t9, t8, r1]), "the whole answer is what placed_rows gave back");
        assert_eq!(calls(&host, &["open_thread"]), vec!["open_thread(w1, t9)".to_string(), "open_thread(w1, t7)".to_string()], "a record that holds its end reads no transcript");
    });
}

// Q2: session-title.test.ts:147
#[test]
fn a_row_with_no_title_waits_for_its_read_and_a_titled_one_does_not() {
    run_local(async {
        let (host, area) = host();
        put_row(&host, "r1", "w1", json!({ "threadId": "t1", "status": "completed" }));
        put_row(&host, "r2", "w1", json!({ "threadId": "t2", "status": "completed", "harnessTitle": "Building the server" }));
        put_row(&host, "r3", "w2", json!({ "threadId": "t3", "status": "completed" }));
        host.hooks.set::<dyn Fn(&str) -> bool>("SessionsCtx::booting", Rc::new(|w: &str| w == "w2"));
        let titled_in: Rc<RefCell<Vec<Vec<String>>>> = Rc::default();
        let seen = titled_in.clone();
        host.hooks.set::<dyn Fn(Vec<SessionView>) -> Vec<SessionView>>(
            "SessionsCtx::title_rows",
            Rc::new(move |rows: Vec<SessionView>| {
                seen.borrow_mut().push(rows.iter().map(|r| r.id.clone()).collect());
                rows
            }),
        );
        let (open_first, first_gate) = oneshot::channel::<()>();
        let first_gate = first_gate.shared();
        let (open_second, second_gate) = oneshot::channel::<()>();
        let second_gate = second_gate.shared();
        let started: Rc<RefCell<Vec<(String, bool)>>> = Rc::default();
        let seen = started.clone();
        host.hooks.set::<dyn Fn(SessionView, bool) -> LocalBoxFuture<'static, Result<(), HostError>>>(
            "SessionsCtx::refresh_title",
            Rc::new(move |v: SessionView, ended: bool| {
                seen.borrow_mut().push((v.id.clone(), ended));
                let gate = if v.id == "r1" { first_gate.clone() } else { second_gate.clone() };
                async move {
                    let _ = gate.await;
                    Ok(())
                }
                .boxed_local()
            }),
        );

        let listing = tokio::task::spawn_local({
            let area = area.clone();
            async move { area.list_rows(None, None).await }
        });
        settle().await;
        assert_eq!(*titled_in.borrow(), vec![vec!["r1".to_string(), "r2".to_string()]], "a row on a workspace still booting is not asked");
        assert_eq!(*started.borrow(), vec![("r1".to_string(), false), ("r2".to_string(), false)], "both reads went out, the titled one unawaited");
        assert!(!listing.is_finished(), "the untitled row blocks the listing on its read");

        let _ = open_first.send(());
        settle().await;
        assert!(listing.is_finished(), "the titled row's read is still open and the listing does not wait on it");
        let rows = listing.await.expect("the task ran").expect("a listing");
        assert_eq!(rows.iter().map(|r| r.id.as_str()).collect::<Vec<_>>(), vec!["r1", "r2", "r3"]);
        let _ = open_second.send(());
    });
}

// Q4: runtime-history.test.ts:38, :290
#[test]
fn history_holds_only_threads_the_caller_drives() {
    run_local(async {
        let (host, area) = host();
        let events = vec![
            ev("session.end", Some("t1"), json!({ "sawResult": true, "turnId": "a" })),
            ev("session.end", Some("t2"), json!({ "sawResult": true, "turnId": "b" })),
            ev("session.end", None, json!({ "sawResult": true, "turnId": "c" })),
            ev("session.end", Some("t3"), json!({ "sawResult": true, "turnId": "d" })),
        ];
        let held = events.clone();
        host.hooks.set::<OpenTranscript>("SessionsCtx::open_transcript", Rc::new(move |_w: &str| {
            let held = held.clone();
            async move { Ok(held) }.boxed_local()
        }));
        host.hooks.set::<DrivesThread>("SessionsCtx::drives_thread", drives_only(&["t1", "t3"]));

        let got = area.history_of("w1", Some(scoped("t1"))).await.expect("a history");
        assert_eq!(json_of(&got), json_of(&vec![events[0].clone(), events[3].clone()]));
        assert_eq!(calls(&host, &["ready", "entry_of_with"]), vec!["ready".to_string()], "a workspace the tree stands on is read as it is");
    });
}

// Q4
#[test]
fn history_of_a_workspace_the_tree_does_not_stand_on_is_read_through_entry_of() {
    run_local(async {
        let (host, area) = host();
        host.hooks.set::<TreeStandsOn>("SessionsCtx::tree_stands_on", Rc::new(|_w: &str, _o: Option<&Caller>, _t: Option<TreeTalk>| false));
        let asked: Rc<RefCell<Vec<(String, bool, bool)>>> = Rc::default();
        let seen = asked.clone();
        host.hooks.set::<EntryOfWith>(
            "SessionsCtx::entry_of_with",
            Rc::new(move |w: &str, _o: Option<&Caller>, being_made: bool, now: bool| {
                seen.borrow_mut().push((w.to_string(), being_made, now));
                async move { Err(refusal("no such workspace w9", Some(Kind::NotFound))) }.boxed_local()
            }),
        );
        let err = area.history_of("w9", Some(scoped("t1"))).await.expect_err("the entry's refusal");
        assert_eq!(said(&err), ("no such workspace w9".to_string(), None, Some(Kind::NotFound)), "its refusal is the caller's");
        assert_eq!(*asked.borrow(), vec![("w9".to_string(), true, false)], "read as a workspace being made, not at once");
        assert_eq!(calls(&host, &["open_transcript"]), Vec::<String>::new());
    });
}

// Q5: thread-head.test.ts:306
#[test]
fn whole_history_refuses_a_window() {
    run_local(async {
        let (host, area) = host();
        let asks = [
            HistoryAsk { thread_id: None, before: None, limit: None },
            HistoryAsk { thread_id: Some("t1".into()), before: Some(10), limit: None },
            HistoryAsk { thread_id: Some("t1".into()), before: None, limit: Some(50) },
        ];
        for ask in asks {
            let err = area.thread_history_of("w1", ask, None).await.expect_err("a refusal");
            let want = usage_refusal("whole reads one thread's every event.", "Name the thread with threadId, and no before or limit.");
            assert_eq!(said(&err), said(&want));
        }
        assert_eq!(calls(&host, &["ready", "open_thread"]), Vec::<String>::new(), "refused before ready and before anything is read");
    });
}

// Q5
#[test]
fn whole_history_of_a_thread_not_driven_is_empty() {
    run_local(async {
        let (host, area) = host();
        host.recorded.borrow_mut().push(end_ev("t1"));
        host.hooks.set::<DrivesThread>("SessionsCtx::drives_thread", drives_only(&["t1"]));
        let ask = |t: &str| HistoryAsk { thread_id: Some(t.into()), before: None, limit: None };

        let none = area.thread_history_of("w1", ask("t2"), Some(scoped("t1"))).await.expect("an empty history");
        assert_eq!(json_of(&none), json!([]));
        assert_eq!(calls(&host, &["opened_words", "open_thread"]), Vec::<String>::new(), "a thread not driven is not opened");

        let got = area.thread_history_of("w1", ask("t1"), Some(scoped("t1"))).await.expect("the thread's events");
        assert_eq!(json_of(&got), json_of(&vec![end_ev("t1")]));
        assert_eq!(calls(&host, &["ready", "opened_words", "open_thread"]), vec!["ready".to_string(), "ready".to_string(), "opened_words(w1, t1)".to_string(), "open_thread(w1, t1)".to_string()]);
    });
}

// Q6: thread-head.test.ts:345
#[test]
fn a_page_of_a_thread_not_driven_is_empty_at_the_index_pos() {
    run_local(async {
        let (host, area) = host();
        host.hooks.set::<DrivesThread>("SessionsCtx::drives_thread", drives_only(&["t1"]));
        let window = || PageWindow { thread_id: "elsewhere".into(), before: None, limit: None };

        let without = area.page_of("w1", window(), Some(scoped("t1"))).await.expect("an empty page");
        assert_eq!(json_of(&without), json!({ "events": [], "pos": 0, "total": 0 }), "no index reads as position 0");

        host.hooks.set::<dyn Fn(&str) -> Option<u64>>("SessionsCtx::index_pos", Rc::new(|w: &str| if w == "w1" { Some(42) } else { None }));
        let with = area.page_of("w1", window(), Some(scoped("t1"))).await.expect("an empty page");
        assert_eq!(json_of(&with), json!({ "events": [], "pos": 42, "total": 0 }));
        assert_eq!(calls(&host, &["opened_words", "read_page"]), Vec::<String>::new());
    });
}

// Q6: thread-head.test.ts:306, :333
#[test]
fn a_page_reads_two_hundred_events_and_four_hundred_kib() {
    run_local(async {
        let (host, area) = host();
        let windows: Rc<RefCell<Vec<(String, String, ReadWindow)>>> = Rc::default();
        let seen = windows.clone();
        let page: HistoryPage = serde_json::from_value(json!({
            "events": [ev("session.end", Some("t1"), json!({ "sawResult": true, "pos": 5 }))], "pos": 5, "total": 9,
        }))
        .expect("a page");
        let answer = page.clone();
        host.hooks.set::<ReadPage>(
            "SessionsCtx::read_page",
            Rc::new(move |w: &str, t: &str, window: ReadWindow| {
                seen.borrow_mut().push((w.to_string(), t.to_string(), window));
                let answer = answer.clone();
                async move { Ok(answer) }.boxed_local()
            }),
        );

        let first = area.page_of("w1", PageWindow { thread_id: "t1".into(), before: None, limit: None }, None).await.expect("a page");
        let back = area.page_of("w1", PageWindow { thread_id: "t1".into(), before: Some(500), limit: Some(1000) }, None).await.expect("a page");
        assert_eq!(json_of(&first), json_of(&page));
        assert_eq!(json_of(&back), json_of(&page));
        assert_eq!(
            *windows.borrow(),
            vec![
                ("w1".to_string(), "t1".to_string(), ReadWindow { before: None, limit: Some(HISTORY_PAGE_EVENTS), bytes: HISTORY_PAGE_BYTES, strict: false, head_shape: false }),
                ("w1".to_string(), "t1".to_string(), ReadWindow { before: Some(500), limit: Some(1000), bytes: HISTORY_PAGE_BYTES, strict: false, head_shape: false }),
            ]
        );
        assert_eq!((HISTORY_PAGE_EVENTS, HISTORY_PAGE_BYTES), (200, 409_600), "the default page is 200 events under 400 KiB");
        assert_eq!(calls(&host, &["opened_words"]), vec!["opened_words(w1, t1)".to_string(), "opened_words(w1, t1)".to_string()]);
    });
}

fn head_page() -> HistoryPage {
    serde_json::from_value(json!({
        "events": [ev("session.end", Some("t1"), json!({ "sawResult": true, "pos": 12 }))], "pos": 12, "total": 4, "trimmed": true,
    }))
    .expect("a page")
}

// Q7: thread-head.test.ts:153
#[test]
fn head_leaves_the_events_what_the_facts_leave() {
    run_local(async {
        let (host, area) = host();
        let held = facts("row-1", "w1", json!({ "threadId": "t1", "title": "日本語 ✓ réponse", "status": "running", "turnId": "turn-9" }));
        let given = held.clone();
        host.hooks.set::<dyn Fn(&str) -> Option<ThreadFacts>>("SessionsCtx::thread_facts", Rc::new(move |_t: &str| Some(given.clone())));
        let reads: Rc<RefCell<Vec<(String, String, ReadWindow)>>> = Rc::default();
        let seen = reads.clone();
        host.hooks.set::<ReadPage>(
            "SessionsCtx::read_page",
            Rc::new(move |w: &str, t: &str, window: ReadWindow| {
                seen.borrow_mut().push((w.to_string(), t.to_string(), window));
                async move { Ok(head_page()) }.boxed_local()
            }),
        );
        let reached: Rc<RefCell<Vec<(Option<String>, String, Option<TreeTalk>, bool)>>> = Rc::default();
        let seen = reached.clone();
        host.hooks.set::<EntryOfRow>(
            "SessionsCtx::entry_of_row",
            Rc::new(move |t: Option<&str>, w: &str, _o: Option<&Caller>, talk: Option<TreeTalk>, being_made: bool| {
                seen.borrow_mut().push((t.map(String::from), w.to_string(), talk, being_made));
                async move { Ok(Some(entry("w1", json!({})) as Entry)) }.boxed_local()
            }),
        );

        let head = area.head_of("row-1", Some(scoped("t1")), false).await.expect("a head");
        let room = HEAD_BYTES - serde_json::to_string(&held).expect("facts serialize").len() as u64 - 100;
        assert_eq!(
            *reads.borrow(),
            vec![("w1".to_string(), "t1".to_string(), ReadWindow { before: None, limit: None, bytes: room, strict: true, head_shape: true })],
            "the facts' own thread, the head's bytes less the facts' UTF-8 bytes less 100, no limit"
        );
        assert_eq!(*reached.borrow(), vec![(Some("t1".to_string()), "w1".to_string(), None, true)], "reached as a thread being made");
        assert_eq!(json_of(&head), json!({
            "facts": json_of(&held), "events": json_of(&head_page().events), "pos": 12, "total": 4, "trimmed": true,
        }));
        assert_eq!(calls(&host, &["thread_facts", "facts_after_read"]), vec!["thread_facts(row-1)".to_string()]);
    });
}

// Q7
#[test]
fn head_reads_the_facts_after_the_read_where_asked() {
    run_local(async {
        let (host, area) = host();
        let held = facts("t1", "w1", json!({}));
        let given = held.clone();
        host.hooks.set::<dyn Fn(&str, Option<&Caller>) -> LocalBoxFuture<'static, Result<Option<ThreadFacts>, HostError>>>(
            "SessionsCtx::facts_after_read",
            Rc::new(move |_t: &str, _o: Option<&Caller>| {
                let given = given.clone();
                async move { Ok(Some(given)) }.boxed_local()
            }),
        );
        let reads: Rc<RefCell<Vec<(String, String)>>> = Rc::default();
        let seen = reads.clone();
        host.hooks.set::<ReadPage>(
            "SessionsCtx::read_page",
            Rc::new(move |w: &str, t: &str, _window: ReadWindow| {
                seen.borrow_mut().push((w.to_string(), t.to_string()));
                async move { Ok(head_page()) }.boxed_local()
            }),
        );
        let head = area.head_of("t1", None, true).await.expect("a head");
        assert_eq!(head.facts, held, "the facts read after the read");
        assert_eq!(*reads.borrow(), vec![("w1".to_string(), "t1".to_string())], "a facts record with no threadId reads the thread asked for");
        assert_eq!(calls(&host, &["thread_facts", "facts_after_read"]), vec!["facts_after_read(t1)".to_string()]);
    });
}

// Q7: thread-head.test.ts:209
#[test]
fn head_of_a_thread_not_reached_is_not_found() {
    run_local(async {
        let (host, area) = host();
        let want = || not_found_refusal(&format!("no thread {}", thread_word("no-such-thread-xyz")));

        let none = area.head_of("no-such-thread-xyz", None, false).await.expect_err("no facts");
        assert_eq!(said(&none), said(&want()));

        host.hooks.set::<dyn Fn(&str) -> Option<ThreadFacts>>("SessionsCtx::thread_facts", Rc::new(|t: &str| Some(facts(t, "w1", json!({})))));
        host.hooks.set::<EntryOfRow>(
            "SessionsCtx::entry_of_row",
            Rc::new(|_t: Option<&str>, _w: &str, _o: Option<&Caller>, _talk: Option<TreeTalk>, _b: bool| async move { Ok(None) }.boxed_local()),
        );
        let unreached = area.head_of("no-such-thread-xyz", Some(scoped("t1")), false).await.expect_err("a thread the caller cannot reach");
        assert_eq!(said(&unreached), said(&want()));
        assert_eq!(calls(&host, &["read_page"]), Vec::<String>::new());
    });
}

fn kept_images(host: &FakeHost, bytes: Option<Vec<u8>>) -> Rc<RefCell<Vec<(String, String)>>> {
    let reads: Rc<RefCell<Vec<(String, String)>>> = Rc::default();
    let seen = reads.clone();
    host.hooks.set::<dyn Fn(&str, &str) -> LocalBoxFuture<'static, Result<Option<Value>, HostError>>>(
        "SessionsCtx::store_get",
        Rc::new(move |c: &str, id: &str| {
            seen.borrow_mut().push((c.to_string(), id.to_string()));
            async move { Ok(Some(json!({ "threads": { "t1": [{ "key": "t1.req_a.0", "mediaType": "image/png" }] } }))) }.boxed_local()
        }),
    );
    host.hooks.set::<dyn Fn(&str, &str) -> LocalBoxFuture<'static, Result<Option<Vec<u8>>, HostError>>>(
        "SessionsCtx::store_get_blob",
        Rc::new(move |_c: &str, _k: &str| {
            let bytes = bytes.clone();
            async move { Ok(bytes) }.boxed_local()
        }),
    );
    reads
}

// Q8: session-images.test.ts:179
#[test]
fn an_attachment_answers_its_bytes_in_base64() {
    run_local(async {
        let (host, area) = host();
        kept_images(&host, Some(vec![7; 5]));
        let got = area.attachment_of("w1", "t1", "req_a", 0, None).await.expect("the image");
        assert_eq!(json_of(&got), json!({ "mediaType": "image/png", "bytes": "BwcHBwc=" }));
        assert_eq!(
            calls(&host, &["ready", "kept_writes", "store_get", "store_get_blob"]),
            vec![
                "ready".to_string(),
                "kept_writes(w1)".to_string(),
                format!("store_get({ATTACHMENT_KEYS}, w1)"),
                format!("store_get_blob({ATTACHMENTS}, t1.req_a.0)"),
            ],
            "after the workspace's kept writes, read by the thread's key list then by the key's blob"
        );
    });
}

// Q8: session-images.test.ts:180
#[test]
fn an_attachment_of_another_thread_is_not_found() {
    run_local(async {
        let (host, area) = host();
        let reads = kept_images(&host, Some(vec![7; 5]));
        host.hooks.set::<DrivesThread>("SessionsCtx::drives_thread", drives_only(&["t1"]));
        let origin = Some(scoped("t1"));
        let gone = |index: u32| said(&not_found_refusal(&format!("no image {} kept on that message", index + 1)));

        // Another index, another send, a thread the caller does not drive, and a request id no key can be made of.
        let e = area.attachment_of("w1", "t1", "req_a", 1, origin.clone()).await.expect_err("no second image");
        assert_eq!(said(&e), gone(1));
        let e = area.attachment_of("w1", "t1", "req_b", 0, origin.clone()).await.expect_err("no such send");
        assert_eq!(said(&e), gone(0));
        let before = reads.borrow().len();
        let e = area.attachment_of("w1", "t2", "req_a", 0, origin.clone()).await.expect_err("a thread not driven");
        assert_eq!(said(&e), gone(0));
        let e = area.attachment_of("w1", "t1", "../req_a", 2, origin.clone()).await.expect_err("an id no key is made of");
        assert_eq!(said(&e), gone(2));
        assert_eq!(reads.borrow().len(), before, "neither of the last two reads the store");

        // A key written whose bytes never landed reads as not found too.
        kept_images(&host, None);
        let e = area.attachment_of("w1", "t1", "req_a", 0, origin).await.expect_err("no blob");
        assert_eq!(said(&e), gone(0));
    });
}

fn hit(w: &str, t: &str, snippet: &str) -> SearchHit {
    SearchHit { workspace_id: w.into(), thread_id: t.into(), snippet: snippet.into() }
}

fn hit_json(w: &str, t: &str, snippet: &str) -> Value {
    json!({ "workspaceId": w, "threadId": t, "snippet": snippet })
}

type SearchIn = dyn Fn(&str, &str, &dyn Fn(&str) -> bool, &mut Vec<SearchedHit>) -> u32;

// Q9: session-search.test.ts:38, :68
#[test]
fn search_trims_and_lowercases_and_answers_newest_first() {
    run_local(async {
        let (host, area) = host();
        host.hooks.set::<dyn Fn() -> Vec<String>>("SessionsCtx::index_workspaces", Rc::new(|| vec!["w1".to_string(), "w2".to_string(), "w3".to_string()]));
        // w2's tree does not stand there and its entry refuses; w3's tree does not stand there but its entry answers.
        host.hooks.set::<TreeStandsOn>("SessionsCtx::tree_stands_on", Rc::new(|w: &str, _o: Option<&Caller>, _t: Option<TreeTalk>| w == "w1"));
        let entries: Rc<RefCell<Vec<(String, bool, bool)>>> = Rc::default();
        let seen = entries.clone();
        host.hooks.set::<EntryOfWith>(
            "SessionsCtx::entry_of_with",
            Rc::new(move |w: &str, _o: Option<&Caller>, being_made: bool, now: bool| {
                seen.borrow_mut().push((w.to_string(), being_made, now));
                let refused = w == "w2";
                async move { if refused { Err(refusal("no such workspace w2", Some(Kind::NotFound))) } else { Ok(entry("w3", json!({})) as Entry) } }.boxed_local()
            }),
        );
        host.hooks.set::<DrivesThread>("SessionsCtx::drives_thread", drives_only(&["t1"]));
        let searched: Rc<RefCell<Vec<(String, String, bool, bool)>>> = Rc::default();
        let seen = searched.clone();
        host.hooks.set::<SearchIn>(
            "SessionsCtx::search_in",
            Rc::new(move |w: &str, words: &str, drives: &dyn Fn(&str) -> bool, found: &mut Vec<SearchedHit>| {
                seen.borrow_mut().push((w.to_string(), words.to_string(), drives("t1"), drives("t2")));
                let push = |found: &mut Vec<SearchedHit>, t: &str, snippet: &str, last: f64| found.push(SearchedHit { hit: hit(w, t, snippet), last: last.into() });
                if w == "w1" {
                    push(found, "t1", "oldest", 100.0);
                    push(found, "t1", "newest", 300.0);
                } else {
                    push(found, "t3", "first tie", 200.0);
                    push(found, "t4", "second tie", 200.0);
                }
                0
            }),
        );

        let got = area.search_for("  CaNoNiCaL  ", Some(scoped("t1"))).await.expect("hits");
        assert_eq!(
            json_of(&got),
            json!({ "hits": [hit_json("w1", "t1", "newest"), hit_json("w3", "t3", "first tie"), hit_json("w3", "t4", "second tie"), hit_json("w1", "t1", "oldest")] }),
            "newest first by last, equal ones in the order found"
        );
        assert_eq!(
            *searched.borrow(),
            vec![("w1".to_string(), "canonical".to_string(), true, false), ("w3".to_string(), "canonical".to_string(), true, false)],
            "the words trimmed and lowercased, the workspace the caller may not read skipped, drives read through drives_thread"
        );
        assert_eq!(*entries.borrow(), vec![("w2".to_string(), false, true), ("w3".to_string(), false, true)], "read at once, not as a workspace being made");
    });
}

// Q9: session-search.test.ts:62
#[test]
fn a_blank_search_finds_nothing() {
    run_local(async {
        let (host, area) = host();
        host.hooks.set::<dyn Fn() -> Vec<String>>("SessionsCtx::index_workspaces", Rc::new(|| vec!["w1".to_string()]));
        let got = area.search_for("   ", None).await.expect("an empty answer");
        assert_eq!(json_of(&got), json!({ "hits": [] }));
        assert_eq!(host.hooks.calls(), vec!["ready".to_string()], "it waits for ready and searches nothing");
    });
}

// Q9
#[test]
fn unsearched_is_said_only_above_zero() {
    run_local(async {
        let (host, area) = host();
        host.hooks.set::<dyn Fn() -> Vec<String>>("SessionsCtx::index_workspaces", Rc::new(|| vec!["w1".to_string(), "w2".to_string()]));
        host.hooks.set::<SearchIn>("SessionsCtx::search_in", Rc::new(|_w: &str, _words: &str, _d: &dyn Fn(&str) -> bool, _f: &mut Vec<SearchedHit>| 0));
        let none = area.search_for("zebra", None).await.expect("hits");
        assert_eq!(json_of(&none), json!({ "hits": [] }), "no unsearched key at zero");

        host.hooks.set::<SearchIn>(
            "SessionsCtx::search_in",
            Rc::new(|w: &str, _words: &str, _d: &dyn Fn(&str) -> bool, _f: &mut Vec<SearchedHit>| if w == "w1" { 2 } else { 1 }),
        );
        let some = area.search_for("zebra", None).await.expect("hits");
        assert_eq!(json_of(&some), json!({ "hits": [], "unsearched": 3 }), "the counts of every workspace add up");
    });
}
