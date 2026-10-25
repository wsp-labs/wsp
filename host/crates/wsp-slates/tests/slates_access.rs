// SPDX-License-Identifier: AGPL-3.0-only
//! Unit slates_access (spec/slates_access.md): who may touch which slate (invariant 3), restart recovery
//! (invariant 2), turn snapshots and rewinds, a window's hold and a thread's forget, on the fake runtime. Every case
//! that writes a slate needs the slate module (`pending::slate`) and stays red until `wsp-slate` exists.
mod support;

use std::rc::Rc;

use serde_json::{json, Value};
use support::{as_thread, local, own_group, sleep_ms, FakeHost, MemStore, THREAD, WORKSPACE};
use wsp_slates::pending::code_of;
use wsp_slates::pending::wire::{SlateEmpty, SlateView};
use wsp_slates::slates::{ApprovalScope, ApproveParams, CatalogParams, EventParams, ReadParams, SlateTarget, Slates, StateParams, WriteParams};
use wsp_wire::types::slate_wire::{SlateBy, SlateCause};
use wsp_wire::views::workspace::Caller;
use wsp_words::exit::{refusal_parts, Kind};

const CHILD: &str = "22222222-bbbb-4ccc-8ddd-000000000002";
const STRANGER: &str = "33333333-cccc-4ddd-8eee-000000000003";

const SETUP_SLOW: &str = r#"<slate title="Long">
  <value name="fired" start={0} />
  <secret name="token" />
  <value name="step" start={2} />
  <run name="slow" cmd="sleep 30" timeout={60} />
  <when done={$slow} do={set($fired, $fired + 1)} />
  <column>
    <input id="tok" label="Token" value={$token} kind="password" />
    <button id="wait" label="Wait" onPress={start($slow)} />
  </column>
</slate>"#;

const TICKER: &str = r#"<slate title="Ticker">
  <value name="ticks" start={0} />
  <value name="at" start={0} />
  <run name="tick" cmd="sleep 1; echo tick" every={60} timeout={20} />
  <when done={$tick} do={set($ticks, $ticks + 1)} />
  <column>
    <text id="clock">clock {time.now}</text>
    <text id="since">since {ago($at)}</text>
  </column>
</slate>"#;

fn text(t: &str) -> WriteParams {
    WriteParams { text: Some(t.into()), ..Default::default() }
}

fn window(thread: &str) -> SlateTarget {
    SlateTarget { thread_id: Some(thread.into()), turn_token: None }
}

fn agent() -> Caller {
    as_thread(THREAD, THREAD)
}

async fn view(s: &Rc<dyn Slates>) -> SlateView {
    s.get(THREAD).await.unwrap().expect("no slate")
}

fn value(v: &SlateView, name: &str) -> Value {
    v.values.get(name).cloned().unwrap_or(Value::Null)
}

fn state_of(v: &SlateView, name: &str) -> String {
    value(v, name)["state"].as_str().unwrap_or("").to_string()
}

/// Settles and reads the view until `check` holds, or fails after `ms`.
async fn wait_for(s: &Rc<dyn Slates>, ms: u64, check: impl Fn(&SlateView) -> bool) -> SlateView {
    let end = std::time::Instant::now() + std::time::Duration::from_millis(ms);
    loop {
        s.settled().await;
        let v = view(s).await;
        if check(&v) {
            return v;
        }
        assert!(std::time::Instant::now() < end, "never held; values {:?}", v.values);
        sleep_ms(30).await;
    }
}

async fn press(s: &Rc<dyn Slates>, piece: &str, request: &str) -> wsp_slates::pending::wire::SlateEventAnswer {
    let version = view(s).await.version;
    s.event(EventParams { thread_id: THREAD.into(), version, piece: piece.into(), event: "press".into(), request_id: request.into(), scope: None, row_action: None }).await.unwrap()
}

async fn approve(s: &Rc<dyn Slates>, key: &str, scope: ApprovalScope) {
    s.approve(ApproveParams { thread_id: THREAD.into(), key: key.into(), scope }).await.unwrap();
}

mod targets {
    use super::*;

    const Z803: &str = "Z803 sub-thread: a thread another thread started has no slate; tell the thread that started you what to show, and it puts it on its own";

    // invariant 3, slates_access Behaviour 7: slates.test.ts:915
    #[test]
    fn invariant_3_a_thread_another_thread_started_has_no_slate_every_tool_refuses_it() {
        let root = support::temp_root("wsp-slates-sub-");
        local(async {
            let host = FakeHost::new(root.path(), MemStore::new());
            host.add_thread(CHILD, THREAD, Some(&host.folder().display().to_string()));
            let s = host.slates();
            let child = as_thread(CHILD, THREAD);
            let e = s.write(text("<slate><column><text>x</text></column></slate>"), Some(&child)).await.unwrap_err();
            assert_eq!((e.message.as_str(), e.kind.clone(), code_of(&e).as_deref()), (Z803, Some(Kind::Usage), Some("Z803")));
            assert_eq!(s.read(ReadParams::default(), Some(&child)).await.unwrap_err().message, Z803);
            let values = Some(indexmap::IndexMap::from([("$x".to_string(), json!(1))]));
            assert_eq!(s.state(StateParams { values, ..Default::default() }, Some(&child)).await.unwrap_err().message, Z803);
            assert_eq!(s.catalog(CatalogParams::default(), Some(&child)).await.unwrap_err().message, Z803);
            assert!(s.get(CHILD).await.unwrap().is_none());
            assert_eq!(s.write(text("<slate><column><text>x</text></column></slate>"), Some(&agent())).await.unwrap().version, 1);
        });
    }

    // invariant 3, slates_access Behaviour 7: a token writes its own slate whatever thread is sent (slates.ts:488-490)
    #[test]
    fn invariant_3_a_token_writes_only_its_own_thread_s_slate() {
        let root = support::temp_root("wsp-slates-own-");
        local(async {
            let host = FakeHost::new(root.path(), MemStore::new());
            host.add_thread(STRANGER, STRANGER, None);
            let s = host.slates();
            let e = s.write(WriteParams { target: window(STRANGER), ..text("<slate><column /></slate>") }, Some(&agent())).await.unwrap_err();
            assert_eq!(e.message, "Z800 thread-not-yours: a thread writes its own slate, and thread 33333333 is not this one");
            assert_eq!((e.kind.clone(), code_of(&e).as_deref()), (Some(Kind::Usage), Some("Z800")));
            assert!(s.get(STRANGER).await.unwrap().is_none());
            let same = s.write(WriteParams { target: window(THREAD), ..text("<slate><column /></slate>") }, Some(&agent())).await.unwrap();
            assert_eq!(same.version, 1);
        });
    }

    // invariant 3, slates_access Behaviour 7: a token reads only the threads under it (slates.ts:491)
    #[test]
    fn invariant_3_a_token_reads_a_thread_under_it_and_no_other() {
        let root = support::temp_root("wsp-slates-under-");
        local(async {
            let host = FakeHost::new(root.path(), MemStore::new());
            host.add_thread(CHILD, THREAD, None);
            host.add_thread(STRANGER, STRANGER, None);
            let s = host.slates();
            let e = s.read(ReadParams { target: window(STRANGER), ..Default::default() }, Some(&agent())).await.unwrap_err();
            assert_eq!((e.message.as_str(), e.kind.clone(), code_of(&e).as_deref()), ("no thread 33333333", Some(Kind::NotFound), Some("Z801")));
            let under = s.read(ReadParams { target: window(CHILD), ..Default::default() }, Some(&agent())).await.unwrap();
            assert_eq!((under.version, under.text.as_str()), (0, "slate v0, empty: write one with slate_write"));
        });
    }

    // slates_access Behaviour 7: with no token, a known thread id, a turn token, or a refusal (slates.ts:494-499)
    #[test]
    fn with_no_token_the_thread_is_named_by_id_or_by_a_turn_token() {
        let root = support::temp_root("wsp-slates-name-");
        local(async {
            let host = FakeHost::new(root.path(), MemStore::new());
            host.tokens.borrow_mut().insert("turn-token-1".into(), THREAD.into());
            let s = host.slates();
            let e = s.read(ReadParams { target: window(STRANGER), ..Default::default() }, None).await.unwrap_err();
            assert_eq!((e.message.as_str(), e.kind), ("no thread 33333333", Some(Kind::NotFound)));
            let none = s.read(ReadParams::default(), Some(&support::here())).await.unwrap_err();
            let parts = refusal_parts(&none);
            assert_eq!(parts.said, "a slate belongs to a thread, and this line names none and runs inside no turn.");
            assert_eq!((parts.fix.as_deref(), none.kind), (Some("Name the thread by id, as wsp threads lists it."), Some(Kind::Usage)));
            let by_token = s.read(ReadParams { target: SlateTarget { thread_id: None, turn_token: Some("turn-token-1".into()) }, ..Default::default() }, None).await.unwrap();
            assert_eq!(by_token.version, 0);
        });
    }

    // slates_access Behaviour 8: a thread with no slate refuses what needs one with Z802 (slates.ts:508-511)
    #[test]
    fn a_thread_with_no_slate_refuses_what_needs_one() {
        let root = support::temp_root("wsp-slates-z802-");
        local(async {
            let host = FakeHost::new(root.path(), MemStore::new());
            let s = host.slates();
            let said = "Z802 no-slate: this thread has no slate yet; write one with slate_write";
            for e in [
                s.revoke(THREAD, "k").await.unwrap_err(),
                s.cancel(THREAD, "r").await.unwrap_err(),
                s.approve(ApproveParams { thread_id: THREAD.into(), key: "k".into(), scope: ApprovalScope::Once }).await.unwrap_err(),
                s.event(EventParams { thread_id: THREAD.into(), version: 0, piece: "p".into(), event: "press".into(), request_id: "r1".into(), scope: None, row_action: None }).await.unwrap_err(),
                s.image(THREAD, "a.png", None).await.unwrap_err(),
            ] {
                assert_eq!((e.message.as_str(), e.kind.clone(), code_of(&e).as_deref()), (said, Some(Kind::Usage), Some("Z802")));
            }
            assert!(s.get(THREAD).await.unwrap().is_none());
            assert!(host.store.row("slates", THREAD).is_none());
        });
    }

    // slates_access Behaviour 9: a write at a version the slate is not at is refused as a conflict (slates.ts:519-521)
    #[test]
    fn a_write_at_a_version_behind_is_refused() {
        let root = support::temp_root("wsp-slates-v750-");
        local(async {
            let host = FakeHost::new(root.path(), MemStore::new());
            let s = host.slates();
            s.write(text(TICKER), Some(&agent())).await.unwrap();
            let e = s.write(WriteParams { if_version: Some(0), ..text(TICKER) }, Some(&agent())).await.unwrap_err();
            assert_eq!((e.message.as_str(), e.kind.clone(), code_of(&e).as_deref()), ("V750 version-behind: the slate is at version 1, not 0; read it and write again", Some(Kind::Conflict), Some("V750")));
            assert_eq!(view(&s).await.version, 1);
        });
    }
}

mod restart {
    use super::*;

    // invariant 2, rule R7, slates_access Behaviour 6: slates.test.ts:189-216
    #[test]
    fn invariant_2_a_run_the_restart_cut_off_fails_once_and_its_done_fires_once() {
        own_group();
        let root = support::temp_root("wsp-slates-restart-");
        local(async {
            let store = MemStore::new();
            let host = FakeHost::new(root.path(), store.clone());
            let s = host.slates();
            s.write(text(SETUP_SLOW), Some(&agent())).await.unwrap();
            s.state(StateParams { target: window(THREAD), values: Some(indexmap::IndexMap::from([("$token".to_string(), json!("vcl_tok_9f8e7d6c5b4a3210"))])), ..Default::default() }, None).await.unwrap();
            let wait = press(&s, "wait", "r-wait").await;
            approve(&s, &wait.ask.unwrap().key(), ApprovalScope::Thread).await;
            wait_for(&s, 10_000, |v| state_of(v, "slow") == "running").await;
            s.settled().await;
            s.close();

            let again = host.again();
            let s2 = again.slates();
            s2.ready().await.unwrap();
            s2.settled().await;
            let restarted = view(&s2).await;
            assert_eq!(state_of(&restarted, "slow"), "failed");
            assert_eq!(value(&restarted, "slow")["why"], json!("the host restarted while it ran"));
            assert_eq!(value(&restarted, "slow")["exit"], Value::Null);
            assert_eq!(value(&restarted, "fired"), json!(1));
            assert_eq!(value(&restarted, "token")["set"], json!(false));
            assert_eq!(value(&restarted, "step"), json!(2));
            s2.close();

            let third = host.again();
            let s3 = third.slates();
            s3.ready().await.unwrap();
            s3.settled().await;
            assert_eq!(value(&view(&s3).await, "fired"), json!(1));
        });
    }

    // rule R7, slates_access Behaviour 5: only schema 2 is loaded, and revision reads as version where absent (slates.ts:433)
    #[test]
    fn ready_loads_schema_2_records_and_reads_a_missing_revision_as_the_version() {
        let root = support::temp_root("wsp-slates-ready-");
        local(async {
            let store = MemStore::new();
            let rec = |thread: &str, schema: i64| {
                json!({"threadId": thread, "workspaceId": WORKSPACE, "rootThreadId": thread, "schema": schema, "version": 3, "document": null, "values": {"n": 1}, "empty": "cleared", "turns": {}, "kept": {}, "comments": [], "approvals": {}, "problems": [], "shownOnce": true, "updatedAt": 5})
            };
            store.rows.borrow_mut().entry("slates".into()).or_default().insert(THREAD.into(), rec(THREAD, 2));
            store.rows.borrow_mut().entry("slates".into()).or_default().insert(STRANGER.into(), rec(STRANGER, 1));
            let host = FakeHost::new(root.path(), store);
            let s = host.slates();
            s.ready().await.unwrap();
            let v = view(&s).await;
            assert_eq!((v.version, v.revision, v.empty, v.shown_once), (3, 3, Some(SlateEmpty::Cleared), true));
            assert_eq!(v.values.get("n"), Some(&json!(1)));
            assert!(s.get(STRANGER).await.unwrap().is_none());
        });
    }

    // slates_access Behaviour 4, 6: slates.test.ts:930 (a timer that ticks as the host boots runs in the thread's folder)
    #[test]
    fn a_timer_that_ticks_as_the_host_boots_starts_its_run_in_the_thread_s_folder() {
        own_group();
        let root = support::temp_root("wsp-slates-boot-");
        local(async {
            let store = MemStore::new();
            let host = FakeHost::new(root.path(), store.clone());
            let inner = host.folder().join("cell");
            std::fs::create_dir_all(&inner).unwrap();
            host.threads.borrow_mut().get_mut(THREAD).unwrap().folder = Some(inner.display().to_string());
            let s = host.slates();
            s.write(text(r#"<slate title="Where"><run name="where" cmd="pwd" every={3600} always timeout={20} /><column><text id="t">{$where.out}</text></column></slate>"#), Some(&agent())).await.unwrap();
            let asked = wait_for(&s, 5_000, |v| v.asks.len() == 1).await;
            approve(&s, &asked.asks[0].key(), ApprovalScope::Thread).await;
            wait_for(&s, 10_000, |v| state_of(v, "where") == "done" && value(v, "where")["runs"] == json!(1)).await;
            s.close();

            let again = host.again();
            again.threads.borrow_mut().get_mut(THREAD).unwrap().folder = Some(inner.display().to_string());
            let s2 = again.slates();
            s2.ready().await.unwrap();
            let v = wait_for(&s2, 10_000, |v| state_of(v, "where") == "done" && value(v, "where")["runs"] == json!(2)).await;
            assert!(again.pushes().iter().any(|p| p.values.get("$where").is_some_and(|w| w["state"] == json!("done") && w["runs"] == json!(2))));
            let out = value(&v, "where")["out"].as_str().unwrap().trim().to_string();
            assert_eq!(std::fs::canonicalize(out).unwrap(), std::fs::canonicalize(&inner).unwrap());
        });
    }

    // slates_access Behaviour 4, 14: slates.test.ts:886 (a settled thread's timers wait for a window)
    #[test]
    fn a_settled_thread_stops_its_timed_runs_and_opening_its_slate_starts_them_at_once() {
        own_group();
        let root = support::temp_root("wsp-slates-settled-");
        local(async {
            let store = MemStore::new();
            let host = FakeHost::new(root.path(), store.clone());
            let s = host.slates();
            s.write(text(r#"<slate title="Feed"><run name="feed" cmd="echo hi" every={3600} always timeout={20} /><column><output run={$feed} /></column></slate>"#), Some(&agent())).await.unwrap();
            let asked = wait_for(&s, 5_000, |v| v.asks.len() == 1).await;
            approve(&s, &asked.asks[0].key(), ApprovalScope::Thread).await;
            wait_for(&s, 10_000, |v| state_of(v, "feed") == "done" && value(v, "feed")["runs"] == json!(1)).await;
            s.close();

            let again = host.again();
            again.settled_threads.borrow_mut().insert(THREAD.into());
            let s2 = again.slates();
            s2.ready().await.unwrap();
            sleep_ms(1_500).await;
            s2.settled().await;
            assert_eq!(value(&view(&s2).await, "feed")["runs"], json!(1));
            let _release = s2.subscribe(THREAD, &[]);
            wait_for(&s2, 10_000, |v| state_of(v, "feed") == "done" && value(v, "feed")["runs"] == json!(2)).await;
        });
    }

    // slates_access Behaviour 14, 6: slates.test.ts:794 (a timed run whose last result failed runs again when shown)
    #[test]
    fn a_timed_run_whose_last_result_failed_runs_again_the_moment_it_is_shown() {
        own_group();
        let root = support::temp_root("wsp-slates-stale-");
        local(async {
            let store = MemStore::new();
            let host = FakeHost::new(root.path(), store.clone());
            let flag = root.path().join("reachable");
            let s = host.slates();
            let doc = format!(r#"<slate title="Feed"><run name="feed" cmd='test -f "{}" && echo "{{}}" || {{ echo unreachable >&2; exit 1; }}' every={{30}} timeout={{20}} /><column><text id="t">{{$feed.state}}</text></column></slate>"#, flag.display());
            s.write(text(&doc), Some(&agent())).await.unwrap();
            let release = s.subscribe(THREAD, &[]);
            let asked = wait_for(&s, 2_000, |v| v.asks.len() == 1).await;
            approve(&s, &asked.asks[0].key(), ApprovalScope::Thread).await;
            let failed = wait_for(&s, 10_000, |v| state_of(v, "feed") == "failed").await;
            assert_eq!(value(&failed, "feed")["err"], json!("unreachable\n"));
            release();
            s.close();

            std::fs::write(&flag, "").unwrap();
            let again = host.again();
            let s2 = again.slates();
            s2.ready().await.unwrap();
            assert_eq!(state_of(&view(&s2).await, "feed"), "failed");
            let _hold = s2.subscribe(THREAD, &[]);
            let done = wait_for(&s2, 5_000, |v| state_of(v, "feed") == "done").await;
            assert_eq!(value(&done, "feed")["json"], json!({}));
        });
    }
}

mod turns {
    use super::*;

    // slates_access Behaviour 11, 12: slates.test.ts:219 (a rewind restores the turn's snapshot)
    #[test]
    fn a_rewind_restores_the_turn_s_snapshot_cancels_a_run_in_flight_with_no_reaction() {
        own_group();
        let root = support::temp_root("wsp-slates-rewind-");
        local(async {
            let host = FakeHost::new(root.path(), MemStore::new());
            let s = host.slates();
            s.turn_ended(THREAD, "turn-1").await.unwrap();
            s.write(text(SETUP_SLOW.replace(r#"<value name="step" start={2} />"#, r#"<value name="step" start={2} /><value name="project" start="" />"#).as_str()), Some(&agent())).await.unwrap();
            let wait = press(&s, "wait", "w1").await;
            approve(&s, &wait.ask.unwrap().key(), ApprovalScope::Thread).await;
            wait_for(&s, 10_000, |v| state_of(v, "slow") == "running").await;
            s.turn_ended(THREAD, "turn-2").await.unwrap();
            s.state(StateParams { target: window(THREAD), values: Some(indexmap::IndexMap::from([("$project".to_string(), json!("later"))])), ..Default::default() }, None).await.unwrap();
            s.turn_ended(THREAD, "turn-3").await.unwrap();
            s.settled().await;
            let before = view(&s).await;

            s.rewound(THREAD, "turn-2", &["turn-3".to_string()]).await.unwrap();
            s.settled().await;
            let rewound = view(&s).await;
            assert_eq!(value(&rewound, "project"), json!(""));
            assert_eq!(state_of(&rewound, "slow"), "cancelled");
            assert_eq!(value(&rewound, "slow")["why"], json!("the thread was rewound"));
            assert_eq!(value(&rewound, "fired"), json!(0));
            assert_eq!((rewound.version, rewound.revision), (before.version + 1, before.revision + 1));
            let restore = host.recorded.borrow().last().cloned().unwrap();
            assert_eq!((restore.cause, restore.by, restore.pieces.len()), (SlateCause::Restore, SlateBy::Host, 0));
            sleep_ms(1_000).await;
            s.settled().await;
            assert_eq!(value(&view(&s).await, "fired"), json!(0));

            s.rewound(THREAD, "turn-1", &["turn-2".to_string()]).await.unwrap();
            let gone = view(&s).await;
            assert!(gone.document.is_none());
            assert_eq!(gone.empty, Some(SlateEmpty::RewoundBefore));
            assert_eq!(gone.approvals.values().map(|a| a.state.clone()).collect::<Vec<_>>(), ["allowed"]);
        });
    }

    // slates_access Behaviour 11, slates_model Behaviour 2: a turn's snapshot is a copy, and a turn ending again moves last
    #[test]
    fn a_turn_s_snapshot_is_a_copy_and_a_turn_ending_again_moves_to_the_end() {
        let root = support::temp_root("wsp-slates-turns-");
        local(async {
            let host = FakeHost::new(root.path(), MemStore::new());
            let s = host.slates();
            s.write(text(TICKER), Some(&agent())).await.unwrap();
            s.turn_ended(THREAD, "a").await.unwrap();
            s.turn_ended(THREAD, "b").await.unwrap();
            s.state(StateParams { target: window(THREAD), values: Some(indexmap::IndexMap::from([("$ticks".to_string(), json!(7))])), ..Default::default() }, None).await.unwrap();
            s.turn_ended(THREAD, "a").await.unwrap();
            let stored = host.store.row("slates", THREAD).unwrap();
            assert_eq!(stored["turns"].as_object().unwrap().keys().cloned().collect::<Vec<_>>(), ["b", "a"]);
            let rev_b = stored["turns"]["b"]["revision"].as_i64().unwrap().to_string();
            assert_eq!(stored["kept"][&rev_b]["values"]["ticks"], json!(0));
            s.rewound(THREAD, "b", &[]).await.unwrap();
            assert_eq!(value(&view(&s).await, "ticks"), json!(0));
        });
    }

    // slates_access Behaviour 11: a turn of a thread with no slate writes nothing
    #[test]
    fn a_turn_of_a_thread_with_no_slate_writes_nothing() {
        let root = support::temp_root("wsp-slates-noturn-");
        local(async {
            let host = FakeHost::new(root.path(), MemStore::new());
            let s = host.slates();
            s.turn_ended(THREAD, "a").await.unwrap();
            s.rewound(THREAD, "a", &[]).await.unwrap();
            assert!(host.store.row("slates", THREAD).is_none());
            assert!(s.get(THREAD).await.unwrap().is_none());
            assert!(host.recorded.borrow().is_empty());
        });
    }
}

mod windows {
    use super::*;

    // slates_access Behaviour 14, 10: slates.test.ts:270 (a timer fires at once when shown, its ticks move no version)
    #[test]
    fn a_timer_fires_at_once_when_shown_and_its_ticks_move_no_version() {
        own_group();
        let root = support::temp_root("wsp-slates-timer-");
        local(async {
            let host = FakeHost::new(root.path(), MemStore::new());
            let s = host.slates();
            assert_eq!(s.write(text(TICKER), Some(&agent())).await.unwrap().version, 1);
            let release = s.subscribe(THREAD, &[]);
            let asked = wait_for(&s, 2_000, |v| state_of(v, "tick") == "held").await;
            assert_eq!(asked.asks.iter().map(|a| a.run()).collect::<Vec<_>>(), ["tick"]);
            approve(&s, &asked.asks[0].key(), ApprovalScope::Thread).await;
            let mid = wait_for(&s, 5_000, |v| state_of(v, "tick") == "running").await;
            assert_eq!(mid.version, 1);
            let patched = s.write(WriteParams { if_version: Some(1), ..text(r#"<props id="since" tone="good" />"#) }, Some(&agent())).await.unwrap();
            assert_eq!(patched.version, 2);
            let after = wait_for(&s, 10_000, |v| value(v, "ticks") == json!(1)).await;
            assert_eq!(after.version, 2);
            assert!(after.revision > mid.revision);
            assert!(after.problems.is_empty());
            let pushes = host.pushes();
            let versions: std::collections::BTreeSet<i64> = pushes.iter().map(|p| p.version.0 .0 as i64).collect();
            assert_eq!(versions, [1, 2].into_iter().collect());
            let revisions: Vec<f64> = pushes.iter().map(|p| p.revision.0 .0).collect();
            let mut sorted = revisions.clone();
            sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
            assert_eq!(revisions, sorted);
            release();
        });
    }

    // slates_access Behaviour 14: a release runs once, and the last hold's release tells both tables not shown
    #[test]
    fn a_release_runs_once_and_holds_count_per_source() {
        let root = support::temp_root("wsp-slates-holds-");
        local(async {
            let host = FakeHost::new(root.path(), MemStore::new());
            let s = host.slates();
            s.write(text(TICKER), Some(&agent())).await.unwrap();
            let pr = s.subscribe(THREAD, &["pr".to_string()]);
            assert_eq!(*host.watched.borrow(), [WORKSPACE]);
            assert!(s.watches_pr(WORKSPACE));
            let pr2 = s.subscribe(THREAD, &["pr".to_string()]);
            pr();
            pr();
            assert!(s.watches_pr(WORKSPACE));
            pr2();
            assert!(!s.watches_pr(WORKSPACE));
            assert!(!s.watches_pr("ws_other"));
        });
    }

    // slates_access Behaviour 18: a document naming pr.checks watches the pull request
    #[test]
    fn a_slate_that_binds_the_pull_request_s_checks_watches_it() {
        let root = support::temp_root("wsp-slates-pr-");
        local(async {
            let host = FakeHost::new(root.path(), MemStore::new());
            let s = host.slates();
            s.write(text(r#"<slate><column><text id="c">{len(pr.checks)}</text></column></slate>"#), Some(&agent())).await.unwrap();
            assert!(s.watches_pr(WORKSPACE));
            assert_eq!(*host.watched.borrow(), [WORKSPACE]);
        });
    }

    // slates_access Behaviour 15: shown sets shownOnce once
    #[test]
    fn shown_marks_the_slate_seen_once() {
        let root = support::temp_root("wsp-slates-shown-");
        local(async {
            let host = FakeHost::new(root.path(), MemStore::new());
            let s = host.slates();
            s.shown(THREAD).await.unwrap();
            assert!(host.store.row("slates", THREAD).is_none());
            s.write(text(TICKER), Some(&agent())).await.unwrap();
            assert!(!view(&s).await.shown_once);
            s.shown(THREAD).await.unwrap();
            let at = host.store.row("slates", THREAD).unwrap()["updatedAt"].clone();
            assert_eq!(host.store.row("slates", THREAD).unwrap()["shownOnce"], json!(true));
            sleep_ms(5).await;
            s.shown(THREAD).await.unwrap();
            assert_eq!(host.store.row("slates", THREAD).unwrap()["updatedAt"], at);
        });
    }
}

mod forget {
    use super::*;

    // slates_access Behaviour 13: slates.test.ts:1136 (deleting a thread deletes its slate, folder and runs)
    #[test]
    fn deleting_a_thread_deletes_its_slate_its_folder_and_its_runs_a_running_one_killed() {
        own_group();
        let root = support::temp_root("wsp-slates-delete-");
        local(async {
            let host = FakeHost::new(root.path(), MemStore::new());
            let s = host.slates();
            let marker = root.path().join("finished");
            let doc = format!(r#"<slate title="Long"><value name="go" start={{0}} /><run name="long" cmd='sleep 2; touch "{}"' timeout={{20}} /><when change={{$go}} do={{start($long)}} /><column><output run={{$long}} /></column><file name="note.txt">{{`kept`}}</file></slate>"#, marker.display());
            s.write(text(&doc), Some(&agent())).await.unwrap();
            s.state(StateParams { target: window(THREAD), values: Some(indexmap::IndexMap::from([("$go".to_string(), json!(1))])), ..Default::default() }, None).await.unwrap();
            let asked = view(&s).await;
            approve(&s, &asked.asks[0].key(), ApprovalScope::Thread).await;
            wait_for(&s, 10_000, |v| state_of(v, "long") == "running").await;
            let dir = host.slates_dir().join(THREAD);
            assert!(host.store.row("slates", THREAD).is_some());

            s.forget(THREAD, false).await.unwrap();
            assert!(s.get(THREAD).await.unwrap().is_none());
            assert!(host.store.row("slates", THREAD).is_none());
            assert!(!dir.exists());
            sleep_ms(3_000).await;
            assert!(!marker.exists());
        });
    }

    // slates_access Behaviour 13: forget drops the write budget too: the next write is a first
    #[test]
    fn a_forgotten_thread_s_write_budget_starts_again() {
        let root = support::temp_root("wsp-slates-forget-budget-");
        local(async {
            let clock = support::ManualClock::new(1_000_000);
            let host = FakeHost::with_clock(root.path(), MemStore::new(), clock.clock());
            let s = host.slates();
            let doc = r#"<slate><column><text id="a">x</text></column></slate>"#;
            for _ in 0..20 {
                s.write(text(doc), Some(&agent())).await.unwrap();
            }
            let e = s.write(text(doc), Some(&agent())).await.unwrap_err();
            assert_eq!((e.message.as_str(), e.kind), ("V751 write-rate: wait 1 s; a slate that rewrites itself constantly is a bug", Some(Kind::Conflict)));
            s.forget(THREAD, false).await.unwrap();
            assert_eq!(s.write(text(doc), Some(&agent())).await.unwrap().version, 1);
        });
    }
}

// slates_access Behaviour 16, 17: settled answers with nothing queued, and close twice is harmless to the caller
#[test]
fn settled_answers_at_once_with_nothing_queued() {
    let root = support::temp_root("wsp-slates-settled-empty-");
    local(async {
        let host = FakeHost::new(root.path(), MemStore::new());
        let s = host.slates();
        s.settled().await;
        assert!(s.get(THREAD).await.unwrap().is_none());
    });
}
