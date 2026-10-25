// SPDX-License-Identifier: AGPL-3.0-only
//! spec/sub_queue.md: the queue every sub-thread's spawn goes through, ported from
//! packages/runtime/test/sub-queue.test.ts and sub-start-events.test.ts's failed first start.
mod machines_b_support;

use std::cell::RefCell;
use std::rc::Rc;

use futures::FutureExt;
use indexmap::IndexMap;
use serde_json::{json, Value};

use machines_b_support::*;
use wsp_core::clock::Cancel;
use wsp_core::types::internal::{KindModule, SessionEntry, SubStartRecord, GONE_REASON, SUB_STARTS};
use wsp_core::types::wiring::{LiveWorkspace, StageReport, SubRecord, WorkspaceRecord};
use wsp_machines::machines::area_error::{AreaError, AreaResult};
use wsp_machines::machines::machines::Entry;
use wsp_machines::machines::sub_queue::*;
use wsp_wire::views::session::ThreadCapWait;
use wsp_wire::views::workspace::Caller;

const TASK: &str = "Fix the flaky upload test in packages/upload.";

fn lead() -> Caller {
    caller("t_lead", "ws_lead", "t_lead")
}

fn wait(running: i64, at_once: i64, why: &str) -> Value {
    json!({ "placeId": "box", "place": "Boat", "running": running, "atOnce": at_once, "why": why })
}

/// A host with `machines` set on Boat, a first sub-thread running there and holding its slot, and the queue.
fn queue(machines: f64) -> (Rc<FakeHost>, Rc<SubQueueArea>) {
    let host = FakeHost::new();
    host.machines_at.borrow_mut().insert("box".to_string(), machines);
    host.put_live(sub_record("ws_first", "running"), FakeMachine::new("bx_first"));
    let area = SubQueueArea::new(host.clone());
    (host, area)
}

/// A spawn's record as sub-spawn writes it, handed to the queue with the wait the queue reads now.
async fn spawned(host: &Rc<FakeHost>, area: &Rc<SubQueueArea>, id: &str, name: &str) -> Rc<RefCell<WorkspaceRecord>> {
    let mut rec = sub_record(id, "running");
    rec["name"] = json!(name);
    rec["machineId"] = json!("");
    let record: WorkspaceRecord = serde_json::from_value(rec).unwrap();
    let cell = Rc::new(RefCell::new(record));
    let limits = host.backend.limits.lock().unwrap().clone();
    let w = area.queue_wait("box", limits.as_ref());
    area.enqueue(&cell, w, false).await.unwrap();
    cell
}

/// The thread's first turn of a queued record, as `run --on boat --detach --notify me` asks it, answered held where
/// the queue holds it (sub-queue.test.ts `startOf`).
struct Started {
    told: Rc<RefCell<Vec<ThreadCapWait>>>,
    turn: tokio::task::JoinHandle<Result<StartedTurn, wsp_words::exit::HostError>>,
}

fn start(area: &Rc<SubQueueArea>, id: &str, thread: &str) -> Started {
    let told = Rc::new(RefCell::new(Vec::new()));
    let keep = told.clone();
    let mut o = serde_json::Map::new();
    o.insert("prompt".into(), json!(format!("This thread runs on a machine of its own.\n\nThe task:\n{TASK}")));
    o.insert("harness".into(), json!("claude"));
    o.insert("title".into(), json!("Second"));
    o.insert("cwd".into(), json!("/root/web"));
    o.insert("turnToken".into(), json!("tok"));
    let mut tree = IndexMap::new();
    tree.insert("parentThreadId".to_string(), json!("t_lead"));
    tree.insert("rootThreadId".to_string(), json!("t_lead"));
    let ask = HoldAsk { workspace_id: id.to_string(), thread_id: thread.to_string(), tree, o, on_held: Some(Rc::new(move |h: HeldTold| keep.borrow_mut().push(h.wait))), notify: Some(vec!["me".to_string()]) };
    let fut = area.hold_queued(ask, Some(&lead()));
    Started { told, turn: tokio::task::spawn_local(fut) }
}

fn phase(host: &FakeHost, id: &str) -> Value {
    host.stored(id).map(|r| r["phase"].clone()).unwrap_or(Value::Null)
}

fn nap(host: &FakeHost, id: &str) {
    let entry = host.live.borrow().get(id).cloned().unwrap();
    entry.record.borrow_mut().phase = serde_json::from_value(json!("napping")).unwrap();
    host.emit_now(json!({ "type": "workspace.napped", "workspaceId": id }));
}

#[test]
fn is_held_as_queued_answered_capped_then_starts_by_itself_first_in_first_out() {
    run(async {
        let (host, area) = queue(1.0);
        let b = spawned(&host, &area, "ws_b", "Second").await;
        let stored = host.stored("ws_b").unwrap();
        assert_eq!((stored["phase"].clone(), stored["machineId"].clone(), stored["queue"]["why"].clone()), (json!("queued"), json!(""), json!("slot")));
        assert_eq!(host.events_of("workspace.created").len(), 1);
        let sb = start(&area, "ws_b", "t_b");
        settle().await;
        assert_eq!(sb.told.borrow().iter().map(text).collect::<Vec<_>>(), vec![text(&wait(1, 1, "slot"))]);
        // Its thread reads Waiting: a running row carrying the count, under the lead.
        let row = host.sessions.borrow().values().find(|s| s.view.workspace_id == "ws_b").cloned().unwrap();
        assert_eq!(serde_json::to_value(&row.view.status).unwrap(), json!("running"));
        assert_eq!(text(row.view.capped.as_ref().unwrap()), text(&wait(1, 1, "slot")));
        assert_eq!((row.view.parent_thread_id.as_deref(), row.view.root_thread_id.as_deref()), (Some("t_lead"), Some("t_lead")));
        assert!(row.queued);
        assert_eq!(host.store.borrow().get(SUB_STARTS).map(|c| c.len()), Some(1));
        let c = spawned(&host, &area, "ws_c", "Third").await;
        let sc = start(&area, "ws_c", "t_c");
        settle().await;
        assert_eq!(sc.told.borrow()[0].why, Some(serde_json::from_value(json!("slot")).unwrap()));
        assert!(host.calls_of("stand_up_queued").is_empty());
        // The first is done and naps: the second, and only it, starts.
        nap(&host, "ws_first");
        settle().await;
        assert_eq!(host.calls_of("stand_up_queued"), vec![vec!["ws_b".to_string(), "waited".to_string()]]);
        let started = sb.turn.await.unwrap().unwrap();
        let start_call = host.calls_of("start_turn")[0].clone();
        let o: Value = serde_json::from_str(&start_call[1]).unwrap();
        assert_eq!(o["as"]["threadId"], json!("t_b"));
        assert_eq!(o["as"]["turnId"], json!(started.turn_id));
        assert_eq!(o["notify"], json!(["me"]));
        assert_eq!(o["prompt"].as_str().unwrap().ends_with(TASK), true);
        assert!(o.get("turnToken").is_none(), "a token never rides the start");
        assert_eq!(start_call[2], "t_lead@ws_lead");
        assert_eq!(b.borrow().queue, None);
        assert_eq!(phase(&host, "ws_c"), json!("queued"));
        // The second naps too: the third starts, and nothing is left written down.
        nap(&host, "ws_b");
        settle().await;
        sc.turn.await.unwrap().unwrap();
        assert_eq!(c.borrow().queue, None);
        assert_eq!(host.store.borrow().get(SUB_STARTS).map(|c| c.len()).unwrap_or(0), 0);
        assert!(host.sessions.borrow().values().all(|s| !s.queued));
    });
}

#[test]
fn lists_its_waiting_row_with_the_head_of_a_long_brief() {
    run(async {
        let (host, area) = queue(1.0);
        spawned(&host, &area, "ws_b", "Second").await;
        let long = format!("{TASK} {}", "Read the ticket and every comment on it before the code. ".repeat(600));
        let told = Rc::new(RefCell::new(Vec::new()));
        let keep = told.clone();
        let mut o = serde_json::Map::new();
        o.insert("prompt".into(), json!(long));
        let ask = HoldAsk { workspace_id: "ws_b".into(), thread_id: "t_b".into(), tree: IndexMap::new(), o, on_held: Some(Rc::new(move |h: HeldTold| keep.borrow_mut().push(h))), notify: None };
        tokio::task::spawn_local(area.hold_queued(ask, Some(&lead())));
        settle().await;
        let row = host.sessions.borrow().values().find(|s| s.view.workspace_id == "ws_b").cloned().unwrap();
        assert_eq!(row.view.prompt.as_deref(), Some(wsp_words::words::turn::kept_opening(&long).as_str()));
        assert_eq!(told.borrow().len(), 1);
    });
}

#[test]
fn holds_a_spawn_with_no_start_left_in_the_minute_and_starts_it_when_a_read_shows_one() {
    run(async {
        let (host, area) = queue(5.0);
        let mut full = paid();
        full.starts.as_mut().unwrap().minute.used = 12.0;
        *host.backend.limits.lock().unwrap() = Some(full);
        let b = spawned(&host, &area, "ws_b", "Second").await;
        assert_eq!(b.borrow().queue.as_ref().unwrap().why, "minute");
        let sb = start(&area, "ws_b", "t_b");
        settle().await;
        assert_eq!(text(&sb.told.borrow()[0]), text(&wait(12, 12, "minute")));
        // Still used at the next read, 10 s on: nothing starts.
        host.clock.advance(QUEUE_MINUTE_READ_MS).await;
        assert!(host.calls_of("stand_up_queued").is_empty());
        // The minute turns: the next read sees a start left.
        *host.backend.limits.lock().unwrap() = Some(paid());
        host.clock.advance(QUEUE_MINUTE_READ_MS).await;
        assert_eq!(host.calls_of("stand_up_queued").len(), 1);
        sb.turn.await.unwrap().unwrap();
    });
}

#[test]
fn a_held_spawn_whose_start_failed_fails_its_row_and_tells_its_targets() {
    run(async {
        let (host, area) = queue(1.0);
        let b = spawned(&host, &area, "ws_b", "Second").await;
        let sb = start(&area, "ws_b", "t_b");
        settle().await;
        host.script("stand_up_queued", Err(failed("fetch failed: Connect Timeout Error")));
        nap(&host, "ws_first");
        settle().await;
        let e = sb.turn.await.unwrap().unwrap_err();
        assert_eq!(e.message, "fetch failed: Connect Timeout Error");
        let r = b.borrow().clone();
        assert_eq!(serde_json::to_value(&r.phase).unwrap(), json!("released"));
        assert_eq!(r.sub.as_ref().unwrap().opening.as_deref().map(|p| p.ends_with(TASK)), Some(true));
        let row = host.sessions.borrow().values().find(|s| s.view.workspace_id == "ws_b").cloned().unwrap();
        assert_eq!(serde_json::to_value(&row.view.status).unwrap(), json!("failed"));
        assert!(row.view.failure.as_deref().unwrap().contains("fetch failed: Connect Timeout Error"));
        assert!(row.view.capped.is_none() && row.launch.is_none());
        assert_eq!(host.calls_of("notify_failed")[0][2], "me");
        // The failed row goes out before the caller hears the refusal.
        let names: Vec<String> = host.calls.borrow().iter().map(|(n, _)| n.clone()).collect();
        assert!(names.iter().position(|n| n == "rows_sent").unwrap() < names.iter().rposition(|n| n == "delete_start").unwrap());
        assert_eq!(host.store.borrow().get(SUB_STARTS).map(|c| c.len()).unwrap_or(0), 0);
    });
}

#[test]
fn drops_a_held_spawn_whose_lead_is_stopped_no_start_spent() {
    run(async {
        let (host, area) = queue(1.0);
        spawned(&host, &area, "ws_b", "Second").await;
        let sb = start(&area, "ws_b", "t_b");
        settle().await;
        let turn = host.sessions.borrow().values().find(|s| s.view.workspace_id == "ws_b").unwrap().turn_id.clone();
        assert!(area.drop_queued(&turn).await);
        let e = sb.turn.await.unwrap().unwrap_err();
        assert_eq!(e.message, wsp_wire::types::sub_thread::SUB_DROPPED_LINE);
        assert_eq!(host.stored("ws_b"), None);
        assert_eq!(host.store.borrow().get(SUB_STARTS).map(|c| c.len()).unwrap_or(0), 0);
        assert!(host.sessions.borrow().values().all(|s| s.view.workspace_id != "ws_b"));
        assert!(host.calls_of("notify_failed").is_empty(), "a stop tells no target");
        // A slot freeing later starts nothing for it.
        nap(&host, "ws_first");
        settle().await;
        assert!(host.calls_of("stand_up_queued").is_empty());
        assert!(!area.drop_queued("no-such-turn").await);
    });
}

#[test]
fn a_held_spawns_record_deleted_goes_with_its_start() {
    run(async {
        let (host, area) = queue(1.0);
        spawned(&host, &area, "ws_b", "Second").await;
        let sb = start(&area, "ws_b", "t_b");
        settle().await;
        host.live.borrow_mut().shift_remove("ws_b");
        host.emit_now(json!({ "type": "workspace.deleted", "workspaceId": "ws_b" }));
        settle().await;
        let e = sb.turn.await.unwrap().unwrap_err();
        assert_eq!(e.message, "Second was deleted before this turn started");
        assert_eq!(host.store.borrow().get(SUB_STARTS).map(|c| c.len()).unwrap_or(0), 0);
        nap(&host, "ws_first");
        settle().await;
        assert!(host.calls_of("stand_up_queued").is_empty());
    });
}

#[test]
fn delete_queued_drops_the_held_start_and_answers_once_gone() {
    run(async {
        let (host, area) = queue(1.0);
        spawned(&host, &area, "ws_b", "Second").await;
        let sb = start(&area, "ws_b", "t_b");
        settle().await;
        assert!(area.delete_queued("ws_nothing").is_none());
        area.delete_queued("ws_b").expect("a held start").await;
        let e = sb.turn.await.unwrap().unwrap_err();
        assert_eq!(e.message, "Second was deleted before this turn started");
        assert_eq!(host.calls_of("drop_workspace"), vec![vec!["ws_b".to_string()]]);
    });
}

#[test]
fn survives_a_restart_queued_record_and_start_come_back_waiting() {
    run(async {
        let (host, area) = queue(1.0);
        spawned(&host, &area, "ws_b", "Second").await;
        let sb = start(&area, "ws_b", "t_b");
        settle().await;
        let turn = host.sessions.borrow().values().find(|s| s.view.workspace_id == "ws_b").unwrap().turn_id.clone();
        drop(sb);
        // The host restarts: the same store, the record live again with no machine, a new queue.
        host.sessions.borrow_mut().clear();
        host.listeners.borrow_mut().clear();
        let again = SubQueueArea::new(host.clone());
        again.resume_queue().await.unwrap();
        settle().await;
        let row = host.sessions.borrow().get(&turn).cloned().expect("the row is back under its turn id");
        assert_eq!(text(row.view.capped.as_ref().unwrap()), text(&wait(1, 1, "slot")));
        assert_eq!(row.view.thread_id.as_deref(), Some("t_b"));
        nap(&host, "ws_first");
        settle().await;
        assert_eq!(host.calls_of("stand_up_queued").len(), 1);
        let o: Value = serde_json::from_str(&host.calls_of("start_turn")[0][1]).unwrap();
        assert!(o["prompt"].as_str().unwrap().ends_with(TASK));
        assert_eq!(o["as"]["turnId"], json!(turn));
        assert_eq!(host.store.borrow().get(SUB_STARTS).map(|c| c.len()).unwrap_or(0), 0);
    });
}

#[test]
fn a_restart_while_a_let_through_spawns_machine_was_made_puts_it_back_in_line() {
    run(async {
        let (host, area) = queue(1.0);
        let b = spawned(&host, &area, "ws_b", "Second").await;
        let _sb = start(&area, "ws_b", "t_b");
        settle().await;
        // The old host had let it through and gone down: queued, its wait over.
        b.borrow_mut().queue = None;
        host.sessions.borrow_mut().clear();
        host.listeners.borrow_mut().clear();
        let again = SubQueueArea::new(host.clone());
        again.resume_queue().await.unwrap();
        settle().await;
        let q = b.borrow().queue.clone().expect("back in line");
        assert_eq!(q.why, "slot");
        assert_eq!(host.stored("ws_b").unwrap()["queue"]["since"], json!(host.store.borrow()[SUB_STARTS]["ws_b"]["at"]));
        nap(&host, "ws_first");
        settle().await;
        assert_eq!(host.calls_of("stand_up_queued").len(), 1);
    });
}

#[test]
fn resume_queue_drops_rows_whose_record_is_not_queued_and_records_with_no_row() {
    run(async {
        let host = FakeHost::new();
        let stale = json!({ "workspaceId": "ws_gone", "threadId": "t", "turnId": "u", "at": "2026-10-12T06:00:00.000Z", "opts": {}, "tree": {} });
        host.store.borrow_mut().entry(SUB_STARTS.to_string()).or_default().insert("ws_gone".into(), stale);
        let mut orphan = sub_record("ws_orphan", "queued");
        orphan["queue"] = json!({ "since": "2026-10-12T06:00:00.000Z", "why": "slot" });
        host.put_live(orphan, FakeMachine::new(""));
        let area = SubQueueArea::new(host.clone());
        area.resume_queue().await.unwrap();
        assert_eq!(host.store.borrow().get(SUB_STARTS).map(|c| c.len()).unwrap_or(0), 0);
        assert_eq!(host.calls_of("drop_workspace"), vec![vec!["ws_orphan".to_string()]]);
    });
}

#[test]
fn slots_count_a_starting_spawn_and_not_a_released_machine() {
    run(async {
        let (host, area) = queue(2.0);
        host.put_live(sub_record("ws_rel", "released"), FakeMachine::new(""));
        assert_eq!(text(&area.slots("box")), text(&wait(1, 2, "slot")));
        let claim = area.claim_start("ws_new", "box");
        assert_eq!(text(&area.slots("box")), text(&wait(2, 2, "slot")));
        assert!(area.wait_on("box", None).is_some());
        (claim.release)();
        assert_eq!(text(&area.slots("box")), text(&wait(1, 2, "slot")));
        // A place the door names nothing for reads its key name and the default count.
        let other = area.slots("solari");
        assert_eq!((other.place.as_str(), other.running.get()), ("Solari", 0));
    });
}

#[test]
fn a_cap_refusal_puts_the_spawn_back_at_the_head_and_asks_again_a_read_later() {
    run(async {
        let (host, area) = queue(5.0);
        let mut full = paid();
        full.machines_at_once = Some(1.0);
        full.active_boxes = Some(1.0);
        *host.backend.limits.lock().unwrap() = Some(full.clone());
        let b = spawned(&host, &area, "ws_b", "Second").await;
        let sb = start(&area, "ws_b", "t_b");
        let c = spawned(&host, &area, "ws_c", "Third").await;
        let sc = start(&area, "ws_c", "t_c");
        settle().await;
        let c_since = c.borrow().queue.clone().unwrap().since;
        // Another client takes Boat's last slot between the read and the create.
        let refusal = wsp_words::exit::HostError::with_kind("Box limit reached", wsp_words::exit::Kind::Concurrency);
        host.script("stand_up_queued", Err(AreaError::Host(refusal)));
        *host.backend.limits.lock().unwrap() = Some(paid());
        host.emit_now(json!({ "type": "place.changed", "placeId": "box" }));
        settle().await;
        assert_eq!(host.calls_of("stand_up_queued").len(), 1);
        // Back at the head: queued again, 1 ms ahead of the third, waiting its read; it did not fail.
        let q = b.borrow().queue.clone().expect("queued again");
        assert_eq!(q.why, "slot");
        assert!(q.since < c_since, "{} before {c_since}", q.since);
        assert!(!sb.turn.is_finished());
        // The third is not let through past it while it waits its read.
        host.emit_now(json!({ "type": "place.changed", "placeId": "box" }));
        settle().await;
        assert_eq!(host.calls_of("stand_up_queued").len(), 1);
        host.clock.advance(QUEUE_SLOT_READ_MS).await;
        assert_eq!(host.calls_of("stand_up_queued")[1][0], "ws_b");
        sb.turn.await.unwrap().unwrap();
        assert_eq!(sc.told.borrow()[0].why, Some(serde_json::from_value(json!("slot")).unwrap()));
    });
}

#[test]
fn two_spawns_asked_together_under_one_machine_one_starts_the_other_queues() {
    run(async {
        let host = FakeHost::new();
        host.machines_at.borrow_mut().insert("box".to_string(), 1.0);
        let area = SubQueueArea::new(host.clone());
        let g = gate();
        *host.stand_up_gate.borrow_mut() = Some(g.clone());
        host.script("stand_up_queued", Ok(json!("gate")));
        let a = spawned(&host, &area, "ws_a", "First").await;
        let sa = start(&area, "ws_a", "t_a");
        settle().await;
        // The first is let through and its machine is being made: its claim holds the slot.
        let b = spawned(&host, &area, "ws_b", "Second").await;
        let sb = start(&area, "ws_b", "t_b");
        settle().await;
        assert!(a.borrow().queue.is_none());
        assert_eq!(b.borrow().queue.as_ref().unwrap().why, "slot");
        assert_eq!(sb.told.borrow().len(), 1);
        open(&g);
        sa.turn.await.unwrap().unwrap();
        assert_eq!(host.calls_of("stand_up_queued").len(), 1);
    });
}

#[test]
fn two_spawns_on_the_minutes_last_start_one_machine_the_other_held_for_the_minute() {
    run(async {
        let host = FakeHost::new();
        let area = SubQueueArea::new(host.clone());
        let mut last = paid();
        last.starts.as_mut().unwrap().minute.used = 11.0;
        *host.backend.limits.lock().unwrap() = Some(last);
        let g = gate();
        *host.stand_up_gate.borrow_mut() = Some(g.clone());
        // The claim is posted at once by the fake; hold the posting back so the start counts here until then.
        host.script("stand_up_queued", Ok(json!("gate")));
        spawned(&host, &area, "ws_a", "First").await;
        let _sa = start(&area, "ws_a", "t_a");
        settle().await;
        let claim = area.claim_start("ws_x", "box");
        let w = area.wait_on("box", host.backend.limits.lock().unwrap().as_ref()).expect("the minute's last start is taken");
        assert_eq!(text(&w), text(&wait(12, 12, "minute")));
        (claim.posted)();
        assert!(area.wait_on("box", host.backend.limits.lock().unwrap().as_ref()).is_none(), "a posted start counts at the cloud, not here");
        (claim.release)();
        open(&g);
    });
}

#[test]
fn passes_over_a_startless_record_and_drops_it_after_the_wait() {
    run(async {
        let (host, area) = queue(1.0);
        spawned(&host, &area, "ws_orphan", "Orphan").await;
        spawned(&host, &area, "ws_c", "Third").await;
        let sc = start(&area, "ws_c", "t_c");
        settle().await;
        nap(&host, "ws_first");
        settle().await;
        assert_eq!(host.calls_of("stand_up_queued"), vec![vec!["ws_c".to_string(), "waited".to_string()]]);
        sc.turn.await.unwrap().unwrap();
        assert_eq!(phase(&host, "ws_orphan"), json!("queued"));
        host.clock.advance(QUEUE_START_WAIT_MS).await;
        assert_eq!(host.calls_of("drop_workspace"), vec![vec!["ws_orphan".to_string()]]);
        assert!(host.warns.borrow().contains(&"workspace ws_orphan: its first turn never came, so its queued spawn is dropped".to_string()));
    });
}

#[test]
fn a_startless_record_alone_is_still_dropped_after_the_wait() {
    run(async {
        let (host, area) = queue(1.0);
        spawned(&host, &area, "ws_orphan", "Orphan").await;
        settle().await;
        assert_eq!(host.clock.due(), vec![host.clock.now() + QUEUE_START_WAIT_MS]);
        host.clock.advance(QUEUE_START_WAIT_MS - 1.0).await;
        assert!(host.calls_of("drop_workspace").is_empty());
        host.clock.advance(1.0).await;
        assert_eq!(host.calls_of("drop_workspace"), vec![vec!["ws_orphan".to_string()]]);
        assert!(host.calls_of("stand_up_queued").is_empty());
    });
}

#[test]
fn a_delete_while_a_let_through_machine_is_made_drops_it_as_it_lands() {
    run(async {
        let (host, area) = queue(1.0);
        spawned(&host, &area, "ws_b", "Second").await;
        let sb = start(&area, "ws_b", "t_b");
        settle().await;
        let g = gate();
        *host.stand_up_gate.borrow_mut() = Some(g.clone());
        host.script("stand_up_queued", Ok(json!("gate")));
        nap(&host, "ws_first");
        settle().await;
        assert_eq!(host.calls_of("stand_up_queued").len(), 1);
        let deleting = area.delete_queued("ws_b").expect("a held start");
        let deleting = tokio::task::spawn_local(deleting);
        settle().await;
        assert!(!deleting.is_finished(), "the stop waits for the machine to land");
        open(&g);
        deleting.await.unwrap();
        assert!(sb.turn.await.unwrap().is_err());
        // The machine went as it landed, cut off first, and no thread started on it.
        let names: Vec<String> = host.calls.borrow().iter().map(|(n, _)| n.clone()).collect();
        assert!(names.iter().position(|n| n == "cut_off").unwrap() < names.iter().position(|n| n == "unfork").unwrap());
        assert!(host.calls_of("start_turn").is_empty());
        assert_eq!(host.calls_of("drop_workspace"), vec![vec!["ws_b".to_string()]]);
    });
}

#[test]
fn a_spawn_behind_the_head_shows_the_heads_wait_and_every_row_takes_its_count() {
    run(async {
        let (host, area) = queue(5.0);
        let mut full = paid();
        full.starts.as_mut().unwrap().minute.used = 12.0;
        *host.backend.limits.lock().unwrap() = Some(full.clone());
        spawned(&host, &area, "ws_b", "Second").await;
        let sb = start(&area, "ws_b", "t_b");
        settle().await;
        spawned(&host, &area, "ws_c", "Third").await;
        let sc = start(&area, "ws_c", "t_c");
        settle().await;
        assert_eq!(text(&sb.told.borrow()[0]), text(&wait(12, 12, "minute")));
        assert_eq!(text(&sc.told.borrow()[0]), text(&wait(12, 12, "minute")));
        full.starts.as_mut().unwrap().minute.used = 13.0;
        *host.backend.limits.lock().unwrap() = Some(full);
        let before = host.recorded.borrow().len();
        host.clock.advance(QUEUE_MINUTE_READ_MS).await;
        let capped: Vec<String> = host.sessions.borrow().values().filter(|s| s.view.workspace_id != "ws_first").map(|s| text(s.view.capped.as_ref().unwrap())).collect();
        assert_eq!(capped, vec![text(&wait(13, 12, "minute")), text(&wait(13, 12, "minute"))]);
        // One session.capped per row that moved, none for a row that did not.
        assert_eq!(host.recorded.borrow()[before..].iter().filter(|e| e["type"] == "session.capped").count(), 2);
    });
}

#[test]
fn a_failed_first_start_stays_released_with_its_opening_kept() {
    run(async {
        let (host, area) = queue(5.0);
        let b = spawned(&host, &area, "ws_b", "Kid").await;
        b.borrow_mut().sub.as_mut().unwrap().spawn = Some(serde_json::from_value(json!({ "startedAt": 1, "log": [] })).unwrap());
        host.script("stand_up_queued", Err(failed("Boat gave two machines in a row that could not run commands")));
        let sb = start(&area, "ws_b", "t_b");
        settle().await;
        assert!(sb.turn.await.unwrap().is_err());
        let s = b.borrow().sub.clone().unwrap();
        assert!(s.opening.as_deref().unwrap().ends_with(TASK));
        assert_eq!(s.spawn.unwrap().asked, s.opening);
        assert_eq!(host.stored("ws_b").unwrap()["phase"], json!("released"));
        assert_eq!(host.calls_of("push_head").last().unwrap(), &vec!["t_b".to_string()]);
    });
}

#[test]
fn queued_orders_by_since_then_id_and_keeps_two_in_one_millisecond_in_order() {
    run(async {
        let (host, area) = queue(1.0);
        let b = spawned(&host, &area, "ws_z", "Second").await;
        let c = spawned(&host, &area, "ws_a", "Third").await;
        let (bs, cs) = (b.borrow().queue.clone().unwrap().since, c.borrow().queue.clone().unwrap().since);
        // Same clock: the second is the first plus 1 ms, so it sorts after whatever its id.
        assert_eq!(bs, "2025-10-09T08:53:20.000Z");
        assert_eq!(cs, "2025-10-09T08:53:20.001Z");
        // The head is the earlier `since`, not the smaller id: let the line move and the first one through is ws_z.
        let _sz = start(&area, "ws_z", "t_z");
        let _sa = start(&area, "ws_a", "t_a");
        settle().await;
        nap(&host, "ws_first");
        settle().await;
        assert_eq!(host.calls_of("stand_up_queued")[0][0], "ws_z");
    });
}

#[test]
fn a_pump_asked_while_one_runs_runs_once_more_not_twice_at_once() {
    run(async {
        let (host, area) = queue(1.0);
        spawned(&host, &area, "ws_b", "Second").await;
        let _sb = start(&area, "ws_b", "t_b");
        settle().await;
        let reads = host.calls_of("limits_of").len();
        let one = area.pump_queue();
        let two = area.pump_queue();
        futures::future::join(one, two).await;
        let more = host.calls_of("limits_of").len() - reads;
        assert!(more <= 2, "one pump and one again, not more: {more}");
    });
}

#[test]
fn attach_queued_answers_the_wait_line_on_every_use() {
    run(async {
        let (host, area) = queue(1.0);
        let cell = Rc::new(RefCell::new(serde_json::from_value::<WorkspaceRecord>(sub_record("ws_q", "queued")).unwrap()));
        area.attach_queued(&cell, None);
        assert_eq!(host.calls_of("attach_absent"), vec![vec!["ws_q".to_string(), "Fix the flaky upload test is waiting for a machine".to_string()]]);
        let w: ThreadCapWait = serde_json::from_value(wait(1, 1, "slot")).unwrap();
        area.attach_queued(&cell, Some(&w));
        assert_eq!(host.calls_of("attach_absent")[1][1], wsp_words::place_state::cap_wait_line(&w));
    });
}

#[test]
fn hold_queued_twice_refuses_the_second() {
    run(async {
        let (host, area) = queue(1.0);
        spawned(&host, &area, "ws_b", "Second").await;
        let _first = start(&area, "ws_b", "t_b");
        settle().await;
        let second = start(&area, "ws_b", "t_b");
        let e = second.turn.await.unwrap().unwrap_err();
        assert_eq!(e.message, wsp_words::sub_thread::sub_waiting_refusal("Second").message);
        assert_eq!(host.sessions.borrow().values().filter(|s| s.view.workspace_id == "ws_b").count(), 1);
    });
}

#[allow(dead_code)]
fn _unused() -> futures::future::LocalBoxFuture<'static, ()> {
    async {}.boxed_local()
}
