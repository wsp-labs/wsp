// SPDX-License-Identifier: AGPL-3.0-only
//! threads_kept (spec/threads_kept.md): agent processes kept between turns and launched ahead. Expected values read
//! off threads.ts:98-301 and wire/limits.ts:45-70 at 65f6b89c7, and thread-keeps-process.test.ts,
//! thread-warm-agent.test.ts, box-keep.test.ts.
#[path = "support_b/mod.rs"]
mod support;

use std::rc::Rc;
use std::sync::atomic::Ordering;

use futures::FutureExt;
use serde_json::json;
use support::threads_ctx::{entry, FakeThreadsCtx};
use support::Lend;
use support::{local, record, settle, FakeKeptAgent};
use wsp_adapters::port::{SessionFile, SessionRenameWrite};
use wsp_core::types::internal::{KeptLaunch, WarmKeep};
use wsp_threads::threads::threads::kept::{agents_kept_on, box_file_line, KeptBox, AGENT_KEEP_MS, AGENT_WARM_MS};
use wsp_threads::threads::threads::{KeptHand, ThreadsArea};

struct Agent {
    closed: std::sync::Arc<std::sync::Mutex<Vec<bool>>>,
    exit: std::sync::Arc<std::sync::Mutex<Option<futures::channel::oneshot::Sender<()>>>>,
    working: std::sync::Arc<std::sync::atomic::AtomicBool>,
}

impl Agent {
    fn closes(&self) -> Vec<bool> {
        self.closed.lock().expect("closed").clone()
    }
}

fn launch(fixed: &str, picks: serde_json::Value) -> KeptLaunch {
    KeptLaunch { fixed: fixed.into(), picks: serde_json::from_value(picks).expect("picks") }
}

/// A hand-over of a turn's process for `workspace`, and the handles a test reads the agent through.
fn hand(workspace: &str, file: Option<SessionFile>, device: Option<&str>) -> (KeptHand, Agent) {
    let agent = FakeKeptAgent::new(file);
    let seen = Agent { closed: agent.closed.clone(), exit: agent.exit.clone(), working: agent.working.clone() };
    (
        KeptHand {
            workspace_id: workspace.into(),
            agent: Box::new(agent),
            pid: None,
            launch: launch("claude|/work|v1", json!({ "model": "opus" })),
            session: Some("s1".into()),
            warm: None,
            turn_token: "tok".into(),
            scope_device_id: device.map(String::from),
            waiting: Default::default(),
        },
        seen,
    )
}

fn warm_hand(workspace: &str) -> (KeptHand, Agent) {
    let (mut h, a) = hand(workspace, None, None);
    h.session = None;
    h.warm = Some(WarmKeep { harness: "claude".into(), claimed: false });
    (h, a)
}

/// A host with a local workspace `ws1`, a workspace `wsb` on the joined computer `box1` (4 GB, linked), and thread
/// records for `threads`.
fn host(threads: &[&str]) -> (Rc<FakeThreadsCtx>, Rc<ThreadsArea>, std::sync::Arc<support::FakeMachine>) {
    let ctx = FakeThreadsCtx::new();
    let (local_ws, _) = entry(json!({ "id": "ws1", "kind": "local" }));
    ctx.add_entry(local_ws);
    let (boxed, machine) = entry(json!({ "id": "wsb", "kind": "local", "place": "box1" }));
    ctx.add_entry(boxed);
    ctx.mem_mb.borrow_mut().insert("box1".into(), 4096.0);
    ctx.linked.borrow_mut().push("box1".into());
    ctx.thread_records.with(|r| {
        for t in threads {
            r.insert(t.to_string(), record("ws1", json!({})));
        }
    });
    let area = ctx.area();
    (ctx, area, machine)
}

fn file() -> SessionFile {
    SessionFile { folder: "/home/u/.claude/projects".into(), name: "s1.jsonl".into(), depth: 1 }
}

fn kept_ids(area: &ThreadsArea) -> Vec<String> {
    area.kept_agents.borrow().keys().cloned().collect()
}

#[test]
fn kept_box_caps_a_joined_computer_off_its_memory() {
    assert_eq!(agents_kept_on(4096.0), 2);
    assert_eq!(agents_kept_on(8192.0), 5);
    assert_eq!(agents_kept_on(1599.0), 0);
    assert_eq!(agents_kept_on(16384.0), 6);
    assert_eq!(agents_kept_on(0.0), 0);
    let (ctx, area, _) = host(&[]);
    assert_eq!(area.kept_box("wsb"), Some(KeptBox { place: "box1".into(), cap: 2 }));
    ctx.mem_mb.borrow_mut().shift_remove("box1");
    assert_eq!(area.kept_box("wsb"), Some(KeptBox { place: "box1".into(), cap: 0 }), "an unknown computer keeps none");
}

#[test]
fn kept_box_is_none_here_and_at_a_provider() {
    let (ctx, area, _) = host(&[]);
    let (cloud, _) = entry(json!({ "id": "wsc", "kind": "cloud" }));
    ctx.add_entry(cloud);
    assert_eq!(area.kept_box("ws1"), None);
    assert_eq!(area.kept_box("wsc"), None);
    assert_eq!(area.kept_box("not-live"), None);
}

#[test]
fn reap_kept_cancels_revokes_and_closes_now() {
    local(async {
        let (ctx, area, _) = host(&["t1"]);
        let (h, agent) = hand("ws1", None, Some("dev-1"));
        assert!(area.hold_kept("t1", h).is_none());
        assert_eq!(ctx.clock.pending(), 1, "the keep's timer");
        area.reap_kept("t1", true);
        settle().await;
        assert!(kept_ids(&area).is_empty());
        assert_eq!(ctx.clock.pending(), 0, "its timer is cancelled");
        assert!(ctx.called().contains(&"revoke dev-1".to_string()));
        assert_eq!(agent.closes(), vec![true]);
        area.reap_kept("t1", true);
        settle().await;
        assert_eq!(agent.closes(), vec![true], "a thread with nothing kept does nothing");
    });
}

#[test]
fn end_kept_waits_for_an_away_computer_and_closes_on_present() {
    local(async {
        let (ctx, area, machine) = host(&["t1"]);
        machine.answer("wc -c", 0, "100\n");
        let (h, agent) = hand("wsb", Some(file()), None);
        assert!(area.hold_kept("t1", h).is_none());
        ctx.linked.borrow_mut().clear();
        area.reap_kept("t1", false);
        settle().await;
        assert!(agent.closes().is_empty(), "nothing reaches an away computer");
        ctx.place_move(wsp_threads::threads::threads::PlaceMove::Present("box2".into()));
        settle().await;
        assert!(agent.closes().is_empty(), "another computer's link says nothing");
        ctx.place_move(wsp_threads::threads::threads::PlaceMove::Present("box1".into()));
        settle().await;
        assert_eq!(agent.closes(), vec![false]);
        assert!(ctx.place_listeners.borrow().is_empty(), "the listener goes once it closed");
    });
}

#[test]
fn end_kept_never_closes_after_the_computer_is_removed() {
    local(async {
        let (ctx, area, _) = host(&["t1"]);
        let (h, agent) = hand("wsb", None, None);
        assert!(area.hold_kept("t1", h).is_none());
        ctx.linked.borrow_mut().clear();
        area.reap_kept("t1", false);
        ctx.place_move(wsp_threads::threads::threads::PlaceMove::Removed("box1".into()));
        ctx.place_move(wsp_threads::threads::threads::PlaceMove::Present("box1".into()));
        settle().await;
        assert!(agent.closes().is_empty());
        assert!(ctx.place_listeners.borrow().is_empty());
    });
}

#[test]
fn end_kept_closes_at_once_while_closing() {
    local(async {
        let (ctx, area, _) = host(&["t1"]);
        let (h, agent) = hand("wsb", None, None);
        assert!(area.hold_kept("t1", h).is_none());
        ctx.linked.borrow_mut().clear();
        ctx.closing.set(true);
        area.reap_kept("t1", false);
        settle().await;
        assert_eq!(agent.closes(), vec![false], "a closing host leaves the run to the next host's sweep and closes now");
    });
}

#[test]
fn end_kept_warns_when_the_revoke_or_close_fails() {
    local(async {
        let (ctx, area, _) = host(&["t1abcdefgh"]);
        let (h, _agent) = hand("ws1", None, Some("dev-1"));
        assert!(area.hold_kept("t1abcdefgh", h).is_none());
        ctx.revoke_fails.set(true);
        let warned = wsp_core::console::capture();
        area.reap_kept("t1abcdefgh", false);
        settle().await;
        assert_eq!(warned.lines(), vec!["the token of thread t1abcdef was not taken away: the device door is shut"]);
    });
}

#[test]
fn box_file_size_sends_the_one_shell_line_and_reads_its_answer() {
    local(async {
        let (_ctx, area, machine) = host(&[]);
        let line = "for f in '/home/u/.claude/projects'/*/'s1.jsonl'; do [ -f \"$f\" ] && { wc -c < \"$f\"; exit 0; }; done; echo -1";
        assert_eq!(box_file_line(&file()), line);
        machine.answer("wc -c", 0, "123\n");
        assert_eq!(area.box_file_size("wsb", &file()).await, Some(123));
        assert_eq!(machine.execs.borrow().last().cloned(), Some((line.to_string(), Some(20_000))));
        machine.rules.borrow_mut().clear();
        machine.answer("wc -c", 0, "-1\n");
        assert_eq!(area.box_file_size("wsb", &file()).await, Some(-1), "no file there");
        machine.rules.borrow_mut().clear();
        machine.answer("wc -c", 0, "12 bytes");
        assert_eq!(area.box_file_size("wsb", &file()).await, Some(12), "parseInt reads the leading number");
        let deep = SessionFile { folder: "/f".into(), name: "x".into(), depth: 3 };
        assert_eq!(box_file_line(&deep), "for f in '/f'/*/*/*/'x'; do [ -f \"$f\" ] && { wc -c < \"$f\"; exit 0; }; done; echo -1");
    });
}

#[test]
fn box_file_size_is_none_on_a_failed_or_unparsable_answer() {
    local(async {
        let (_ctx, area, machine) = host(&[]);
        machine.answer("wc -c", 1, "123");
        assert_eq!(area.box_file_size("wsb", &file()).await, None, "a non-zero exit");
        machine.rules.borrow_mut().clear();
        machine.answer("wc -c", 0, "bytes 12");
        assert_eq!(area.box_file_size("wsb", &file()).await, None);
        machine.rules.borrow_mut().clear();
        machine.fail("wc -c", "the link dropped");
        assert_eq!(area.box_file_size("wsb", &file()).await, None);
        assert_eq!(area.box_file_size("not-live", &file()).await, None);
    });
}

#[test]
fn write_session_restamps_every_process_on_the_session_once_written() {
    local(async {
        let (_ctx, area, machine) = host(&["t1"]);
        machine.answer("wc -c", 0, "100");
        let (h, _agent) = hand("wsb", Some(file()), None);
        assert!(area.hold_kept("t1", h).is_none());
        settle().await;
        machine.rules.borrow_mut().clear();
        machine.answer("wc -c", 0, "150");
        let wrote = area.write_session("s1", async { Ok(SessionRenameWrite::Written) }.boxed_local()).await;
        assert_eq!(wrote.expect("written"), SessionRenameWrite::Written);
        assert_eq!(machine.commands().iter().filter(|c| c.contains("wc -c")).count(), 2, "read at the keep and again after the write");
        let stamped = area.kept_agents.borrow().get("t1").and_then(|k| k.borrow().box_file.clone()).expect("a box stamp");
        assert_eq!(stamped.now_or_never(), Some(Some(150)), "the host's own write is the process's stamp now");
    });
}

#[test]
fn write_session_stamps_nothing_on_a_failed_write() {
    local(async {
        let (_ctx, area, machine) = host(&["t1"]);
        machine.answer("wc -c", 0, "100");
        let (h, _agent) = hand("wsb", Some(file()), None);
        assert!(area.hold_kept("t1", h).is_none());
        settle().await;
        let failed = area.write_session("s1", async { Ok(SessionRenameWrite::Failed { error: "locked".into() }) }.boxed_local()).await;
        assert_eq!(failed.expect("answered"), SessionRenameWrite::Failed { error: "locked".into() });
        let refused = area.write_session("s1", async { Err(support::err("no store")) }.boxed_local()).await;
        assert_eq!(refused.expect_err("its own failure").message, "no store");
        assert_eq!(machine.commands().iter().filter(|c| c.contains("wc -c")).count(), 1);
    });
}

#[test]
fn host_writes_holds_until_every_write_settled() {
    local(async {
        let (_ctx, area, _) = host(&[]);
        let (tx1, rx1) = futures::channel::oneshot::channel::<()>();
        let (tx2, rx2) = futures::channel::oneshot::channel::<()>();
        let first = area.write_session(
            "s9",
            async move {
                let _ = rx1.await;
                Ok(SessionRenameWrite::NoSession)
            }
            .boxed_local(),
        );
        let second = area.write_session(
            "s9",
            async move {
                let _ = rx2.await;
                Ok(SessionRenameWrite::NoSession)
            }
            .boxed_local(),
        );
        let (a, b) = (tokio::task::spawn_local(first), tokio::task::spawn_local(second));
        settle().await;
        assert!(area.host_writes.borrow().contains_key("s9"));
        let _ = tx2.send(());
        let _ = b.await;
        settle().await;
        assert!(area.host_writes.borrow().contains_key("s9"), "the second holder cannot release the first's hold");
        let _ = tx1.send(());
        let _ = a.await;
        settle().await;
        assert!(!area.host_writes.borrow().contains_key("s9"), "gone once every write settled");
    });
}

#[test]
fn kept_check_reaps_when_the_link_is_gone() {
    local(async {
        let (ctx, area, _) = host(&["t1"]);
        let (h, _agent) = hand("wsb", None, None);
        assert!(area.hold_kept("t1", h).is_none());
        ctx.linked.borrow_mut().clear();
        area.kept_check("t1").await;
        assert!(kept_ids(&area).is_empty());
    });
}

#[test]
fn kept_check_reaps_when_the_file_there_changed() {
    local(async {
        let (_ctx, area, machine) = host(&["t1", "t2"]);
        machine.answer("wc -c", 0, "100");
        let (h, _a) = hand("wsb", Some(file()), None);
        assert!(area.hold_kept("t1", h).is_none());
        settle().await;
        area.kept_check("t1").await;
        assert_eq!(kept_ids(&area), vec!["t1"], "the same size keeps it");
        machine.rules.borrow_mut().clear();
        machine.answer("wc -c", 0, "180");
        area.kept_check("t1").await;
        assert!(kept_ids(&area).is_empty(), "the person resumed the session there");
    });
}

#[test]
fn kept_check_keeps_a_working_process_and_an_unread_size() {
    local(async {
        let (_ctx, area, machine) = host(&["t1"]);
        machine.answer("wc -c", 0, "100");
        let (h, agent) = hand("wsb", Some(file()), None);
        assert!(area.hold_kept("t1", h).is_none());
        settle().await;
        machine.rules.borrow_mut().clear();
        machine.answer("wc -c", 0, "180");
        agent.working.store(true, Ordering::SeqCst);
        area.kept_check("t1").await;
        assert_eq!(kept_ids(&area), vec!["t1"], "a working process owns the change");
        agent.working.store(false, Ordering::SeqCst);
        machine.rules.borrow_mut().clear();
        machine.fail("wc -c", "no answer");
        area.kept_check("t1").await;
        assert_eq!(kept_ids(&area), vec!["t1"], "a size that did not read is no change");
        area.kept_check("t-none").await;
    });
}

#[test]
fn take_kept_answers_a_matching_process_untouched() {
    local(async {
        let (ctx, area, _) = host(&["t1"]);
        let (h, agent) = hand("ws1", None, Some("dev-1"));
        assert!(area.hold_kept("t1", h).is_none());
        let taken = area.take_kept("t1", Some(&launch("claude|/work|v1", json!({}))), Some("s1"), None).expect("it matches");
        assert_eq!(taken.borrow().turn_token, "tok");
        settle().await;
        assert!(kept_ids(&area).is_empty());
        assert!(agent.closes().is_empty());
        assert!(!ctx.called().iter().any(|c| c.starts_with("revoke")), "its device stays with the process");
        assert_eq!(ctx.clock.pending(), 0, "its timer is cancelled");
    });
}

#[test]
fn take_kept_reaps_on_another_launch_session_or_file() {
    local(async {
        for (fixed, session, model) in [
            ("other|/work|v1", Some("s1"), "opus"),
            ("claude|/work|v1", Some("s2"), "opus"),
            ("claude|/work|v1", None, "opus"),
            ("claude|/work|v1", Some("s1"), "sonnet"),
        ] {
            let (_ctx, area, _) = host(&["t1"]);
            let (h, agent) = hand("ws1", None, None);
            assert!(area.hold_kept("t1", h).is_none());
            assert!(
                area.take_kept("t1", Some(&launch(fixed, json!({ "model": model }))), session, None).is_none(),
                "{fixed} {session:?} {model}"
            );
            settle().await;
            assert!(kept_ids(&area).is_empty());
            assert_eq!(agent.closes(), vec![false]);
        }
        let dir = tempfile::tempdir().expect("a folder");
        std::fs::write(dir.path().join("s1.jsonl"), b"{}\n").expect("written");
        let on_disk = SessionFile { folder: dir.path().to_string_lossy().into_owned(), name: "s1.jsonl".into(), depth: 0 };
        let (_ctx, area, _) = host(&["t1"]);
        let (h, agent) = hand("ws1", Some(on_disk), None);
        assert!(area.hold_kept("t1", h).is_none());
        std::fs::write(dir.path().join("s1.jsonl"), b"{}\n{}\n").expect("the person resumed it");
        assert!(
            area.take_kept("t1", Some(&launch("claude|/work|v1", json!({}))), Some("s1"), None).is_none(),
            "the file grew since the turn"
        );
        settle().await;
        assert_eq!(agent.closes(), vec![false]);
        assert!(area.take_kept("t1", None, Some("s1"), None).is_none(), "nothing kept");
    });
}

#[test]
fn take_kept_matches_a_warm_process_both_ways() {
    local(async {
        let (_ctx, area, _) = host(&[]);
        let (h, _a) = warm_hand("ws1");
        area.hold_warm("t-w", h);
        let whole_more = launch("claude|/work|v1", json!({ "model": "opus", "effort": "high" }));
        assert!(
            area.take_kept("t-w", Some(&launch("claude|/work|v1", json!({}))), None, Some(&whole_more)).is_none(),
            "the send's whole launch names an effort the warm process was not launched at"
        );
        let (h, _a) = warm_hand("ws1");
        area.hold_warm("t-w", h);
        assert!(
            area.take_kept("t-w", Some(&launch("claude|/work|v1", json!({}))), None, None).is_none(),
            "a warm process needs the whole launch"
        );
        let (h, _a) = warm_hand("ws1");
        area.hold_warm("t-w", h);
        let whole = launch("claude|/work|v1", json!({ "model": "opus" }));
        assert!(
            area.take_kept("t-w", Some(&launch("other", json!({}))), None, Some(&whole)).is_some(),
            "matched on the whole launch both ways, the partial one unread"
        );
    });
}

#[test]
fn hold_kept_refuses_while_closing_or_with_no_record() {
    local(async {
        let (ctx, area, _) = host(&["t1"]);
        let (h, agent) = hand("ws1", None, Some("dev-1"));
        assert!(area.hold_kept("t-unknown", h).is_some(), "a thread with no record keeps nothing");
        settle().await;
        assert_eq!(agent.closes(), vec![false]);
        assert!(!ctx.called().iter().any(|c| c.starts_with("revoke")), "the caller keeps the token");
        ctx.closing.set(true);
        let (h, agent) = hand("ws1", None, None);
        assert!(area.hold_kept("t1", h).is_some());
        settle().await;
        assert_eq!(agent.closes(), vec![false]);
        assert!(kept_ids(&area).is_empty());
    });
}

#[test]
fn hold_kept_replaces_the_threads_older_process() {
    local(async {
        let (ctx, area, _) = host(&["t1"]);
        let (first, old) = hand("ws1", None, Some("dev-old"));
        assert!(area.hold_kept("t1", first).is_none());
        ctx.clock.advance(5);
        let (second, new) = hand("ws1", None, None);
        assert!(area.hold_kept("t1", second).is_none());
        settle().await;
        assert_eq!(old.closes(), vec![false]);
        assert!(new.closes().is_empty());
        assert!(ctx.called().contains(&"revoke dev-old".to_string()));
        assert_eq!(area.kept_agents.borrow().get("t1").map(|k| k.borrow().used_at.0), Some(1_000_005.0));
    });
}

#[test]
fn hold_kept_reads_the_file_size_on_a_joined_computer() {
    local(async {
        let (_ctx, area, machine) = host(&["t1"]);
        machine.answer("wc -c", 0, "77");
        let (h, _a) = hand("wsb", Some(file()), None);
        assert!(area.hold_kept("t1", h).is_none());
        settle().await;
        let kept = area.kept_agents.borrow().get("t1").cloned().expect("kept");
        assert!(kept.borrow().file.is_none(), "no local stamp for a file on another computer");
        assert_eq!(kept.borrow().box_file.clone().expect("read there").now_or_never(), Some(Some(77)));
    });
}

#[test]
fn hold_warm_keeps_two_unclaimed_and_reaps_the_oldest() {
    local(async {
        let (ctx, area, _) = host(&[]);
        let mut agents = vec![];
        for (i, t) in ["w1", "w2", "w3"].iter().enumerate() {
            let (h, a) = warm_hand("ws1");
            area.hold_warm(t, h);
            agents.push(a);
            ctx.clock.advance(10 * (i as i64 + 1));
        }
        settle().await;
        assert_eq!(kept_ids(&area), vec!["w2", "w3"]);
        assert_eq!(agents[0].closes(), vec![false]);
        assert_eq!(ctx.called().iter().filter(|c| *c == "procScopesMoved").count(), 3);
    });
}

#[test]
fn claim_warm_takes_one_once() {
    local(async {
        let (ctx, area, _) = host(&[]);
        let (h, _a) = warm_hand("ws1");
        area.hold_warm("w1", h);
        assert_eq!(area.warm_on("ws1", "claude").map(|(t, _)| t), Some("w1".to_string()));
        assert_eq!(area.warm_on("ws1", "codex").map(|(t, _)| t), None);
        assert_eq!(area.warm_on("wsb", "claude").map(|(t, _)| t), None);
        assert_eq!(area.claim_warm("ws1", "claude"), Some("w1".to_string()));
        assert_eq!(area.claim_warm("ws1", "claude"), None, "taken once, by one send");
        assert_eq!(area.kept_agents.borrow().get("w1").and_then(|k| k.borrow().warm.clone()).map(|w| w.claimed), Some(true));
        assert_eq!(ctx.called().iter().filter(|c| *c == "procScopesMoved").count(), 2);
    });
}

#[test]
fn hold_warm_ends_at_once_while_closing() {
    local(async {
        let (ctx, area, _) = host(&[]);
        ctx.closing.set(true);
        let (mut h, agent) = warm_hand("ws1");
        h.scope_device_id = Some("dev-w".into());
        area.hold_warm("w1", h);
        settle().await;
        assert!(kept_ids(&area).is_empty());
        assert_eq!(agent.closes(), vec![false]);
        assert!(ctx.called().contains(&"revoke dev-w".to_string()));
    });
}

#[test]
fn rewarm_starts_the_window_over() {
    local(async {
        let (ctx, area, _) = host(&[]);
        let (h, agent) = warm_hand("ws1");
        area.hold_warm("w1", h);
        ctx.clock.advance(AGENT_WARM_MS - 1000);
        area.rewarm("w1");
        ctx.clock.advance(AGENT_WARM_MS - 1000);
        settle().await;
        assert_eq!(kept_ids(&area), vec!["w1"], "the composer asked again, so its window started over");
        assert_eq!(area.kept_agents.borrow().get("w1").map(|k| k.borrow().used_at.0), Some(1_000_000.0 + (AGENT_WARM_MS - 1000) as f64));
        ctx.clock.advance(1000);
        settle().await;
        assert!(kept_ids(&area).is_empty());
        assert_eq!(agent.closes(), vec![false]);
    });
}

#[test]
fn the_keep_ends_an_idle_process_at_its_window_and_re_arms_a_working_one() {
    local(async {
        let (ctx, area, _) = host(&["t1"]);
        let (h, agent) = hand("ws1", None, None);
        assert!(area.hold_kept("t1", h).is_none());
        agent.working.store(true, Ordering::SeqCst);
        ctx.clock.advance(AGENT_KEEP_MS);
        settle().await;
        assert_eq!(kept_ids(&area), vec!["t1"], "work it left going holds it");
        agent.working.store(false, Ordering::SeqCst);
        ctx.clock.advance(AGENT_KEEP_MS - 1);
        settle().await;
        assert_eq!(kept_ids(&area), vec!["t1"], "re-armed for the same window");
        ctx.clock.advance(1);
        settle().await;
        assert!(kept_ids(&area).is_empty());
        assert_eq!(agent.closes(), vec![false]);
    });
}

#[test]
fn a_process_that_exits_leaves_the_keep_with_its_device() {
    local(async {
        let (ctx, area, _) = host(&["t1"]);
        let (h, agent) = hand("ws1", None, Some("dev-1"));
        assert!(area.hold_kept("t1", h).is_none());
        let _ = agent.exit.lock().expect("exit").take().expect("once").send(());
        settle().await;
        assert!(kept_ids(&area).is_empty());
        assert!(ctx.called().contains(&"revoke dev-1".to_string()));
        assert!(agent.closes().is_empty(), "nothing is left to close");
        assert_eq!(ctx.clock.pending(), 0);
    });
}

#[test]
fn the_cap_reaps_the_idle_one_kept_longest() {
    local(async {
        let names = ["t0", "t1", "t2", "t3", "t4", "t5", "t6"];
        let (ctx, area, _) = host(&names);
        let mut agents = vec![];
        for t in names {
            let (h, a) = hand("ws1", None, None);
            assert!(area.hold_kept(t, h).is_none());
            agents.push(a);
            ctx.clock.advance(1);
        }
        settle().await;
        assert_eq!(kept_ids(&area), vec!["t1", "t2", "t3", "t4", "t5", "t6"], "six on this computer");
        assert_eq!(agents[0].closes(), vec![false]);
    });
}

#[test]
fn the_cap_counts_this_computer_and_providers_together() {
    local(async {
        let names = ["t0", "t1", "t2", "t3", "t4", "t5", "t6"];
        let (ctx, area, _) = host(&names);
        let (cloud, _) = entry(json!({ "id": "wsc", "kind": "cloud" }));
        ctx.add_entry(cloud);
        for (i, t) in names.iter().enumerate() {
            let (h, _a) = hand(if i % 2 == 0 { "wsc" } else { "ws1" }, None, None);
            assert!(area.hold_kept(t, h).is_none());
            ctx.clock.advance(1);
        }
        assert_eq!(
            kept_ids(&area),
            vec!["t1", "t2", "t3", "t4", "t5", "t6"],
            "a provider workspace stands on no joined computer, so it shares this one's count"
        );
        let (h, _a) = hand("wsb", None, None);
        assert!(area
            .hold_kept("t-box", {
                ctx.thread_records.with(|r| {
                    r.insert("t-box".into(), record("wsb", json!({})));
                });
                h
            })
            .is_none());
        assert_eq!(kept_ids(&area).len(), 7, "the joined computer counts alone");
    });
}

#[test]
fn the_cap_stays_over_when_every_process_works() {
    local(async {
        let names = ["t0", "t1", "t2", "t3", "t4", "t5"];
        let (ctx, area, _) = host(&["t0", "t1", "t2", "t3", "t4", "t5", "t6"]);
        for t in names {
            let (h, a) = hand("ws1", None, None);
            a.working.store(true, Ordering::SeqCst);
            assert!(area.hold_kept(t, h).is_none());
            ctx.clock.advance(1);
        }
        let (h, a) = hand("ws1", None, None);
        a.working.store(true, Ordering::SeqCst);
        assert!(area.hold_kept("t6", h).is_none());
        assert_eq!(kept_ids(&area).len(), 7, "no idle one to end");
    });
}
