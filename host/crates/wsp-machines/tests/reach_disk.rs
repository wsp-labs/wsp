// SPDX-License-Identifier: AGPL-3.0-only
//! spec/reach_disk.md: the status a row is pushed with, the refused nap and stop, and the disk of a machine whose stop
//! snapshots it, ported from packages/runtime/test/disk-watch.test.ts, gone.test.ts and status.test.ts.
mod machines_b_support;

use std::rc::Rc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use serde_json::{json, Value};

use machines_b_support::reach_ctx::*;
use machines_b_support::*;
use wsp_core::types::wiring::WakeAsk;
use wsp_engine::disk_sync::{disk_use_of, DISK_USE_CMD};
use wsp_engine::machine::RootDisk;
use wsp_machines::machines::machines::Entry;
use wsp_machines::machines::reach::{
    ForMachine, ReachArea, ReachCtx, DISK_CLOSE_WAIT_MS, DISK_READ_MS,
};
use wsp_wire::views::workspace::ReachState;
use wsp_wire::wire::capabilities::PauseMode;
use wsp_words::place_word::provider_key_name;
use wsp_words::words::thread::{
    disk_full_line, disk_mark_line, nap_refused_line, stop_refused_line, DiskMark, DiskMove,
};

const WS: &str = "ws_1";

/// Boat's root disk as measured inside a box (2026-10-12), in kB: the 75 GB one read on a small box, the older 12 GB one
/// and the 160 GB one, each as df reports it.
const DISK_75_GB_KB: f64 = 68_928_192.0;
const DISK_12_GB_KB: f64 = 11_004_751.0;
const DISK_160_GB_KB: f64 = 146_730_014.0;

struct W {
    host: Rc<FakeHost>,
    area: Rc<ReachArea>,
    entry: Entry,
    machine: Arc<FakeMachine>,
}

fn world(record: Value) -> W {
    let host = reach_host();
    let machine = FakeMachine::new("bx_1");
    let entry = host.put_live(record, machine.clone());
    let area = reach(&host);
    W {
        host,
        area,
        entry,
        machine,
    }
}

/// Boat's root disk per size: small and default share one, large grows it, xlarge was never measured so it has none.
fn set_root_disks(host: &FakeHost) {
    let mut disks = host.backend.root_disks.lock().unwrap();
    disks.insert(
        "2x4096".to_string(),
        RootDisk {
            name: "small".to_string(),
            disk_gb: 75.0,
        },
    );
    disks.insert(
        "4x8192".to_string(),
        RootDisk {
            name: "default".to_string(),
            disk_gb: 75.0,
        },
    );
    disks.insert(
        "8x16384".to_string(),
        RootDisk {
            name: "large".to_string(),
            disk_gb: 160.0,
        },
    );
}

/// One df answer for a disk of that many kB at that share used.
fn df(disk_kb: f64, pct: f64) -> String {
    let used = (disk_kb * pct / 100.0).round() as i64;
    format!("{} {}", used, disk_kb as i64 - used)
}

/// df answering in turn off `readings`, the last standing once they run out; the counter says how many reads came.
fn df_in_turn(m: &FakeMachine, readings: Vec<String>) -> Arc<AtomicUsize> {
    let n = Arc::new(AtomicUsize::new(0));
    let seen = n.clone();
    m.on(move |cmd| {
        if cmd != DISK_USE_CMD {
            return None;
        }
        let i = seen.fetch_add(1, Ordering::SeqCst);
        Some(ok(&format!("{}\n", readings[i.min(readings.len() - 1)])))
    });
    n
}

/// df answering `first` once and `then` from the second read on.
fn df_after_first(m: &FakeMachine, first: &str, then: &str) -> Arc<AtomicUsize> {
    df_in_turn(m, vec![first.to_string(), then.to_string()])
}

fn swept(m: &FakeMachine) -> Vec<&'static str> {
    m.log()
        .iter()
        .filter_map(|c| {
            if c == SWEEP_STAGED {
                Some("staged")
            } else if c == SWEEP_CACHES {
                Some("caches")
            } else {
                None
            }
        })
        .collect()
}

fn reason_of(df_answer: &str) -> String {
    disk_full_line(disk_use_of(df_answer).expect("a df answer").pct)
}

fn go(thread: &str, name: &str, cpu: f64, mem_mb: f64, disk_gb: f64) -> DiskMove {
    DiskMove {
        thread: thread.to_string(),
        name: name.to_string(),
        cpu,
        mem_mb,
        disk_gb,
    }
}

/// What the area says about a machine read as `first` and then, after the clearing, as `after`.
fn mark(
    machine: &str,
    on: &str,
    first: &str,
    after: &str,
    machine_goes: bool,
    to: Option<DiskMove>,
) -> DiskMark {
    let a = disk_use_of(first).expect("a df answer");
    let b = disk_use_of(after).expect("a df answer");
    DiskMark {
        machine: machine.to_string(),
        on: on.to_string(),
        pct: b.pct,
        used_bytes: b.used_bytes,
        size_bytes: b.used_bytes + b.free_bytes,
        freed_bytes: (a.used_bytes - b.used_bytes).max(0.0),
        machine_goes,
        r#move: to,
    }
}

fn notice(workspace: &str, session: &str, turn: &str, thread: &str, line: &str) -> Value {
    json!({ "type": "session.notice", "workspaceId": workspace, "sessionId": session, "turnId": turn, "threadId": thread, "text": line })
}

fn recorded(host: &FakeHost) -> Vec<Value> {
    host.recorded.borrow().clone()
}

fn warns(host: &FakeHost) -> Vec<String> {
    host.warns.borrow().clone()
}

/// The `reason` each pushed status carried, `None` for a status without one.
fn status_reasons(host: &FakeHost) -> Vec<Option<String>> {
    host.events_of("workspace.status")
        .iter()
        .map(|e| e["status"]["reason"].as_str().map(String::from))
        .collect()
}

#[test]
fn is_read_on_the_beat_cleared_past_90_and_told_again_only_after_falling_5_under() {
    run(async {
        let w = world(plain_record(WS, "web", "running", "bx_1"));
        set_root_disks(&w.host);
        // 50%, then 93% (the crossing), 92% after the clearing, 88% and 91% (never under 85, so nothing new), 84% (under),
        // and 91% a second crossing.
        let readings: Vec<String> = [50.0, 93.0, 92.0, 88.0, 91.0, 84.0, 91.0]
            .iter()
            .map(|p| df(DISK_75_GB_KB, *p))
            .collect();
        let reads = df_in_turn(&w.machine, readings.clone());
        put_session(
            &w.host,
            "turn-1",
            WS,
            Some("t_1"),
            Some(w.host.clock.now() - 60_000.0),
            Some("sess-1"),
        );
        let on = provider_key_name("box");

        // The turn's end: 50% says nothing and arms the beat.
        w.area.read_disk(&w.entry).await;
        assert!(recorded(&w.host).is_empty());
        assert!(w.host.events.borrow().is_empty());
        assert_eq!(w.host.clock.due(), vec![w.host.clock.now() + DISK_READ_MS]);

        // The beat reads 93%: what wsp made is cleared, the disk read again at 92%, the thread told.
        w.host.clock.advance(DISK_READ_MS).await;
        assert_eq!(swept(&w.machine), vec!["staged", "caches"]);
        assert_eq!(
            w.host.calls_of("sweep_leftovers"),
            vec![vec!["bx_1".to_string()]]
        );
        let to = Some(go("t_1", "large", 8.0, 16384.0, 160.0));
        let first_text = disk_mark_line(&mark(
            "web",
            &on,
            &readings[1],
            &readings[2],
            false,
            to.clone(),
        ));
        assert_eq!(
            recorded(&w.host).iter().map(text).collect::<Vec<_>>(),
            vec![text(&notice(WS, "sess-1", "turn-1", "t_1", &first_text))]
        );
        // default keeps small's root disk, so the move offered skips it for large, the first size whose disk is bigger.
        assert!(first_text.contains("wsp cleared 706 MB of its own staging and build caches there"));
        assert!(first_text.contains("Free space on its disk. Or move it to large (8 vCPU, 16 GB, 160 GB disk): wsp resize t_1 --size 8x16 stops it and resumes it at that size, with its files, which ends a turn running there."));
        assert_eq!(
            warns(&w.host),
            vec![format!(
                "web: {}",
                disk_mark_line(&mark("web", &on, &readings[1], &readings[2], false, None))
            )]
        );
        // The row crossed the line and moved with the reading: each push carried its reason.
        assert_eq!(
            status_reasons(&w.host),
            vec![Some(reason_of(&readings[1])), Some(reason_of(&readings[2]))]
        );
        assert_eq!(w.area.disk_reason(&w.entry), Some(reason_of(&readings[2])));

        // 88%: under the line, and the row says so at once; still over 85, so the crossing stays told.
        w.host.clock.advance(DISK_READ_MS).await;
        assert_eq!(w.area.disk_reason(&w.entry), None);
        assert_eq!(status_reasons(&w.host).last().cloned(), Some(None));
        // 91%: past the line again with the crossing still told: nothing cleared, nothing said.
        w.host.clock.advance(DISK_READ_MS).await;
        assert_eq!(w.area.disk_reason(&w.entry), Some(reason_of(&readings[4])));
        assert_eq!(recorded(&w.host).len(), 1);
        assert_eq!(swept(&w.machine), vec!["staged", "caches"]);
        // 84%: under the re-arm line.
        w.host.clock.advance(DISK_READ_MS).await;
        assert_eq!(w.area.disk_reason(&w.entry), None);
        assert_eq!(recorded(&w.host).len(), 1);
        // 91% again: a second crossing is cleared and said again.
        w.host.clock.advance(DISK_READ_MS).await;
        assert_eq!(
            swept(&w.machine),
            vec!["staged", "caches", "staged", "caches"]
        );
        let second_text = disk_mark_line(&mark("web", &on, &readings[6], &readings[6], false, to));
        let all = recorded(&w.host);
        assert_eq!(all.len(), 2);
        assert_eq!(
            text(&all[1]),
            text(&notice(WS, "sess-1", "turn-1", "t_1", &second_text))
        );
        assert!(
            second_text.contains("found none of its own staging or build caches there to clear")
        );
        assert_eq!(reads.load(Ordering::SeqCst), 8);
    });
}

#[test]
fn a_machine_still_clearing_holds_back_no_other_machines_read() {
    run(async {
        let host = reach_host();
        set_root_disks(&host);
        let stuck_m = FakeMachine::new("bx_1");
        let other_m = FakeMachine::new("bx_2");
        let stuck = host.put_live(
            plain_record("ws_web", "web", "running", "bx_1"),
            stuck_m.clone(),
        );
        let other = host.put_live(
            plain_record("ws_api", "api", "running", "bx_2"),
            other_m.clone(),
        );
        let area = reach(&host);
        // Under the line at the turn's end, past it from the first beat on.
        let stuck_reads = df_after_first(&stuck_m, "500000 500000", "950000 50000");
        let other_reads = df_after_first(&other_m, "500000 500000", "950000 50000");
        // The first machine's clearing sits on Boat for minutes; the other's answers at once.
        let held = gate();
        stuck_m.answer(SWEEP_STAGED, Reply::Gated(held.clone(), Box::new(ok(""))));
        put_session(
            &host,
            "turn-w",
            "ws_web",
            Some("t_w"),
            Some(1.0),
            Some("sw"),
        );
        put_session(
            &host,
            "turn-a",
            "ws_api",
            Some("t_a"),
            Some(1.0),
            Some("sa"),
        );
        area.read_disk(&stuck).await;
        area.read_disk(&other).await;
        let reads = || {
            (
                stuck_reads.load(Ordering::SeqCst),
                other_reads.load(Ordering::SeqCst),
            )
        };
        assert_eq!(reads(), (1, 1));

        host.clock.advance(DISK_READ_MS).await;
        assert_eq!(reads(), (2, 3));
        fn told(h: &FakeHost) -> Vec<String> {
            h.recorded
                .borrow()
                .iter()
                .map(|e| e["workspaceId"].as_str().unwrap().to_string())
                .collect()
        }
        assert_eq!(told(&host), vec!["ws_api".to_string()]);
        host.clock.advance(DISK_READ_MS).await;
        host.clock.advance(DISK_READ_MS).await;
        assert_eq!(reads(), (2, 5));

        // The clearing lands: its second reading and its notice come, and the next beat reads the machine again.
        open(&held);
        settle().await;
        assert_eq!(stuck_reads.load(Ordering::SeqCst), 3);
        assert_eq!(
            told(&host),
            vec!["ws_api".to_string(), "ws_web".to_string()]
        );
        host.clock.advance(DISK_READ_MS).await;
        assert_eq!(reads(), (4, 6));
    });
}

#[test]
fn a_host_closing_waits_on_a_read_still_clearing_only_so_long() {
    run(async {
        let w = world(plain_record(WS, "web", "running", "bx_1"));
        df_in_turn(&w.machine, vec!["950000 50000".to_string()]);
        // The clearing never answers, as a sweep can sit for its whole five minute guard.
        w.machine.answer(SWEEP_STAGED, Reply::Never);
        put_session(&w.host, "turn-1", WS, Some("t_1"), Some(1.0), Some("s1"));
        let (a, e) = (w.area.clone(), w.entry.clone());
        let _read = tokio::task::spawn_local(async move { a.read_disk(&e).await });
        settle().await;
        assert!(w.machine.ran(SWEEP_STAGED));
        assert_eq!(w.host.clock.due(), vec![w.host.clock.now() + DISK_READ_MS]);

        let a = w.area.clone();
        let stop = tokio::task::spawn_local(async move { a.stop_disk_watch().await });
        settle().await;
        // The beat is cancelled and only the bounded wait stands.
        assert!(!stop.is_finished());
        assert_eq!(
            w.host.clock.due(),
            vec![w.host.clock.now() + DISK_CLOSE_WAIT_MS]
        );
        w.host.clock.advance(DISK_CLOSE_WAIT_MS - 1.0).await;
        assert!(!stop.is_finished());
        w.host.clock.advance(1.0).await;
        assert!(stop.is_finished());
        stop.await.unwrap();
        assert_eq!(w.host.clock.pending(), 0);
        assert!(recorded(&w.host).is_empty());
    });
}

#[test]
fn offers_the_first_size_up_that_grows_the_disk_and_none_on_large() {
    run(async {
        // A small box on Boat's older 12 GB disk moves to default; one on the 75 GB disk default shares moves to large; on
        // large nothing is offered, since xlarge's disk was never measured.
        let cases = [
            (
                2.0,
                4096.0,
                DISK_12_GB_KB,
                Some(("default", 4.0, 8192.0, 75.0, "4x8")),
            ),
            (
                2.0,
                4096.0,
                DISK_75_GB_KB,
                Some(("large", 8.0, 16384.0, 160.0, "8x16")),
            ),
            (8.0, 16384.0, DISK_160_GB_KB, None),
        ];
        for (cpu, mem_mb, disk_kb, offer) in cases {
            let mut record = plain_record(WS, "web", "running", "bx_1");
            record["size"] = json!({ "cpu": cpu, "memMb": mem_mb });
            let w = world(record);
            set_root_disks(&w.host);
            *w.host.backend.limits.lock().unwrap() = Some(paid());
            let reading = df(disk_kb, 95.0);
            df_in_turn(&w.machine, vec![reading.clone()]);
            put_session(&w.host, "turn-1", WS, Some("t_1"), Some(1.0), Some("s1"));
            w.area.read_disk(&w.entry).await;

            let to = offer.map(|(name, c, m, d, _)| go("t_1", name, c, m, d));
            let line = disk_mark_line(&mark(
                "web",
                &provider_key_name("box"),
                &reading,
                &reading,
                false,
                to,
            ));
            assert_eq!(
                recorded(&w.host).iter().map(text).collect::<Vec<_>>(),
                vec![text(&notice(WS, "s1", "turn-1", "t_1", &line))]
            );
            match offer {
                Some((name, c, m, d, size)) => {
                    let said = format!("Or move it to {name} ({c} vCPU, {} GB, {d} GB disk): wsp resize t_1 --size {size} ", m / 1024.0);
                    assert!(line.contains(&said), "{line}");
                }
                None => assert!(!line.contains("Or move it to"), "{line}"),
            }
            // The plan's limits are read once before the sizes are weighed, as they refresh the sizes.
            assert_eq!(*w.host.backend.limits_reads.lock().unwrap(), 1);
        }
    });
}

#[test]
fn reads_nothing_on_a_machine_whose_stop_keeps_memory() {
    run(async {
        let w = world(plain_record(WS, "web", "running", "bx_1"));
        w.host.backend.capabilities.lock().unwrap().pause_mode = Some(PauseMode::Memory);
        df_in_turn(&w.machine, vec!["950000 50000".to_string()]);
        put_session(&w.host, "turn-1", WS, Some("t_1"), Some(1.0), Some("s1"));
        w.area.read_disk(&w.entry).await;
        w.area.arm_disk_watch();
        assert_eq!(w.host.clock.pending(), 0);
        w.host.clock.advance(DISK_READ_MS).await;
        assert_eq!(w.machine.log(), Vec::<String>::new());
        assert!(recorded(&w.host).is_empty());
        assert!(w.host.events.borrow().is_empty());
        assert!(warns(&w.host).is_empty());
        assert_eq!(w.area.disk_reason(&w.entry), None);
    });
}

#[test]
fn tells_a_sub_threads_machine_it_goes() {
    run(async {
        let w = world(sub_record("ws_kid", "running"));
        let name = "Fix the flaky upload test";
        // The backend offers a plan's limits but no root disks: a size is weighed only where a disk is known.
        *w.host.backend.limits.lock().unwrap() = Some(paid());
        *knobs().provider.borrow_mut() = None;
        // The first reading crosses; the clearing fails; the second reading reads nothing, so the first stands.
        df_in_turn(
            &w.machine,
            vec!["930000 70000".to_string(), "not a reading".to_string()],
        );
        w.host.script("sweep_leftovers", Err(failed("disk busy")));
        put_session(
            &w.host,
            "turn-k",
            "ws_kid",
            Some("t_kid"),
            Some(1.0),
            Some("sk"),
        );
        w.area.read_disk(&w.entry).await;

        let line = disk_mark_line(&mark(
            name,
            &provider_key_name("the provider"),
            "930000 70000",
            "930000 70000",
            true,
            None,
        ));
        assert!(line.contains("or let the thread finish: its machine is deleted once it is done."));
        assert!(line.contains("found none of its own staging or build caches there to clear"));
        assert_eq!(
            recorded(&w.host).iter().map(text).collect::<Vec<_>>(),
            vec![text(&notice("ws_kid", "sk", "turn-k", "t_kid", &line))]
        );
        assert_eq!(
            warns(&w.host),
            vec![
                format!("what wsp made on {name} was not cleared: disk busy"),
                format!("{name}: {line}")
            ]
        );
        assert_eq!(*w.host.backend.limits_reads.lock().unwrap(), 0);
    });
}

#[test]
fn a_nap_refusal_stands_for_its_machine_and_drops_for_another() {
    run(async {
        let w = world(plain_record(WS, "web", "running", "bx_1"));
        assert_eq!(w.area.nap_refused_of(&w.entry), None);
        w.area.nap_refusals.borrow_mut().insert(
            WS.to_string(),
            ForMachine {
                machine_id: "bx_1".to_string(),
                value: "Not pausable".to_string(),
            },
        );
        assert_eq!(
            w.area.nap_refused_of(&w.entry),
            Some("Not pausable".to_string())
        );
        assert_eq!(
            w.area.nap_refused_reason(&w.entry),
            Some(nap_refused_line("Not pausable"))
        );
        assert!(w.area.nap_refusals.borrow().contains_key(WS));

        // A rebuild puts another machine under the record: the refusal reads as none and is taken out of the map.
        let rebuilt = w.host.reattach(&w.entry, FakeMachine::new("bx_2"));
        assert_eq!(w.area.nap_refused_of(&rebuilt), None);
        assert!(!w.area.nap_refusals.borrow().contains_key(WS));
        assert_eq!(w.area.nap_refused_reason(&w.entry), None);
    });
}

#[test]
fn a_stop_refusal_stands_on_the_row_for_its_machine() {
    run(async {
        let w = world(plain_record(WS, "web", "running", "bx_1"));
        let said = "its latest snapshot failed at 2026-10-01T00:40:00Z, and the last that completed was at 2026-09-30T21:02:00Z";
        assert_eq!(w.area.stop_refused_reason(&w.entry), None);
        w.area.stop_refusals.borrow_mut().insert(
            WS.to_string(),
            ForMachine {
                machine_id: "bx_1".to_string(),
                value: said.to_string(),
            },
        );
        assert_eq!(
            w.area.stop_refused_reason(&w.entry),
            Some(stop_refused_line(said))
        );
        // The row built by the poll reads it through the host's own reasons.
        assert_eq!(w.host.row_reason(&w.entry), Some(stop_refused_line(said)));

        // Another machine reads none, and unlike a nap's the stale refusal is left in the map.
        let rebuilt = w.host.reattach(&w.entry, FakeMachine::new("bx_2"));
        assert_eq!(w.area.stop_refused_reason(&rebuilt), None);
        assert!(w.area.stop_refusals.borrow().contains_key(WS));
        assert_eq!(
            w.area.stop_refused_reason(&w.entry),
            Some(stop_refused_line(said))
        );
    });
}

#[test]
fn disk_reason_is_none_at_exactly_90() {
    run(async {
        let w = world(plain_record(WS, "web", "running", "bx_1"));
        df_in_turn(
            &w.machine,
            vec!["900000 100000".to_string(), "910000 90000".to_string()],
        );
        put_session(&w.host, "turn-1", WS, Some("t_1"), Some(1.0), Some("s1"));
        w.area.read_disk(&w.entry).await;
        assert_eq!(w.area.disk_reason(&w.entry), None);
        assert!(recorded(&w.host).is_empty());
        assert!(w.host.events.borrow().is_empty());
        assert!(warns(&w.host).is_empty());
        assert_eq!(swept(&w.machine), Vec::<&str>::new());

        w.area.read_disk(&w.entry).await;
        assert_eq!(
            w.area.disk_reason(&w.entry),
            Some(reason_of("910000 90000"))
        );
        assert_eq!(swept(&w.machine), vec!["staged", "caches"]);
        assert_eq!(recorded(&w.host).len(), 1);
    });
}

#[test]
fn tell_threads_writes_under_the_latest_turn_of_each_thread() {
    run(async {
        let w = world(plain_record(WS, "web", "running", "bx_1"));
        let h = &w.host;
        put_session(h, "a1", WS, Some("t_a"), Some(100.0), Some("ca1"));
        put_session(h, "a2", WS, Some("t_a"), Some(200.0), None);
        put_session(h, "b1", WS, Some("t_b"), Some(150.0), Some("cb1"));
        // A tie goes to the later row in map order.
        put_session(h, "b2", WS, Some("t_b"), Some(150.0), Some("cb2"));
        // A row without a thread is its own thread, named by its row id.
        put_session(h, "c1", WS, None, None, Some("cc1"));
        // A later row that started earlier does not replace the held one.
        put_session(h, "a3", WS, Some("t_a"), Some(50.0), Some("ca3"));
        // Rows with no start time read 0 and tie, so the later wins.
        put_session(h, "e1", WS, Some("t_e"), None, None);
        put_session(h, "e2", WS, Some("t_e"), None, None);
        put_session(h, "d1", "ws_other", Some("t_d"), Some(1.0), Some("cd1"));

        w.area
            .tell_threads(WS, &|thread: &str| format!("note for {thread}"));
        let want = [
            notice(WS, "a2", "a2", "t_a", "note for t_a"),
            notice(WS, "cb2", "b2", "t_b", "note for t_b"),
            notice(WS, "cc1", "c1", "c1", "note for c1"),
            notice(WS, "e2", "e2", "t_e", "note for t_e"),
        ];
        assert_eq!(
            recorded(h).iter().map(text).collect::<Vec<_>>(),
            want.iter().map(text).collect::<Vec<_>>()
        );
    });
}

#[test]
fn reach_of_takes_unreached_then_polled_then_the_kinds_claim() {
    run(async {
        let w = world(plain_record(WS, "web", "running", "bx_1"));
        let k = knobs();
        // Nothing polled and no claim.
        assert_eq!(w.area.reach_of(&w.entry), ReachState::Unsupported);
        // The kind's claim, then the machine's own daemon-answers road.
        k.has_daemon.set(true);
        assert_eq!(w.area.reach_of(&w.entry), ReachState::Reachable);
        k.has_daemon.set(false);
        w.machine.offers.lock().unwrap().daemon_answers = true;
        assert_eq!(w.area.reach_of(&w.entry), ReachState::Reachable);
        w.machine.offers.lock().unwrap().daemon_answers = false;
        assert_eq!(w.area.reach_of(&w.entry), ReachState::Unsupported);

        // A reading the poll made for this machine outranks the claim.
        k.has_daemon.set(true);
        w.area.polled_reach.borrow_mut().insert(
            WS.to_string(),
            ForMachine {
                machine_id: "bx_1".to_string(),
                value: ReachState::NoDaemon,
            },
        );
        assert_eq!(w.area.reach_of(&w.entry), ReachState::NoDaemon);
        // One made for another machine is not this one's.
        k.has_daemon.set(false);
        w.area.polled_reach.borrow_mut().insert(
            WS.to_string(),
            ForMachine {
                machine_id: "bx_0".to_string(),
                value: ReachState::Reachable,
            },
        );
        assert_eq!(w.area.reach_of(&w.entry), ReachState::Unsupported);

        // A line about the daemon outranks them all.
        w.area.polled_reach.borrow_mut().insert(
            WS.to_string(),
            ForMachine {
                machine_id: "bx_1".to_string(),
                value: ReachState::Reachable,
            },
        );
        k.unreached
            .borrow_mut()
            .insert(WS.to_string(), "the daemon is down".to_string());
        assert_eq!(w.area.reach_of(&w.entry), ReachState::Unreachable);
    });
}

#[test]
fn emit_status_carries_the_entrys_facts_in_order() {
    run(async {
        let w = world(plain_record(WS, "web", "running", "bx_1"));
        knobs()
            .idle_at
            .borrow_mut()
            .insert(WS.to_string(), 1_760_000_300_000.0);
        {
            let mut e = w.entry.borrow_mut();
            e.wake_ask = Some(WakeAsk { ask: 1, of: 3 });
            e.checkout = Some(serde_json::from_value(json!({ "branch": "main", "ahead": 1, "behind": 0, "changed": 2, "readAt": 1_760_000_000_000u64 })).unwrap());
            e.pr = Some(
                serde_json::from_value(
                    json!({ "why": "gh is not signed in", "readAt": 1_760_000_000_002u64 }),
                )
                .unwrap(),
            );
            e.tree = Some(
                serde_json::from_value(
                    json!({ "leadBranch": "main", "children": [], "readAt": 1_760_000_000_001u64 }),
                )
                .unwrap(),
            );
        }
        w.area
            .emit_status(&w.entry, ReachState::Reachable, Some("because"))
            .await
            .unwrap();

        let want = json!({
            "type": "workspace.status",
            "status": {
                "id": WS, "name": "web", "phase": "running", "machineState": "running", "reach": { "state": "reachable" },
                "size": { "cpu": 2, "memMb": 4096 }, "rateUsdPerHour": 0.018, "reason": "because",
                "wakeAsk": { "ask": 1, "of": 3 }, "idleAt": 1_760_000_300_000u64,
                "checkout": { "branch": "main", "ahead": 1, "behind": 0, "changed": 2, "readAt": 1_760_000_000_000u64 },
                "pr": { "why": "gh is not signed in", "readAt": 1_760_000_000_002u64 },
                "tree": { "leadBranch": "main", "children": [], "readAt": 1_760_000_000_001u64 }
            }
        });
        assert_eq!(
            w.host
                .events
                .borrow()
                .iter()
                .map(js_text)
                .collect::<Vec<_>>(),
            vec![js_text(&want)]
        );

        // A napping row carries no idle time even where the host holds one, and only the facts it has.
        let nap = w.host.put_live(
            plain_record("ws_2", "api", "napping", "bx_2"),
            FakeMachine::new("bx_2"),
        );
        knobs()
            .idle_at
            .borrow_mut()
            .insert("ws_2".to_string(), 1_760_000_400_000.0);
        w.area
            .emit_status(&nap, ReachState::Napping, None)
            .await
            .unwrap();
        let bare = json!({
            "type": "workspace.status",
            "status": {
                "id": "ws_2", "name": "api", "phase": "napping", "machineState": "paused", "reach": { "state": "napping" },
                "size": { "cpu": 2, "memMb": 4096 }, "rateUsdPerHour": 0.018
            }
        });
        let events = w.host.events.borrow().clone();
        assert_eq!(events.len(), 2);
        assert_eq!(js_text(&events[1]), js_text(&bare));
    });
}

#[test]
fn status_now_and_push_status_read_the_phase() {
    run(async {
        let w = world(plain_record(WS, "web", "running", "bx_1"));
        let nap = w.host.put_live(
            plain_record("ws_2", "api", "napping", "bx_2"),
            FakeMachine::new("bx_2"),
        );
        knobs().has_daemon.set(true);
        fn reach_of(h: &FakeHost, i: usize) -> String {
            h.events.borrow()[i]["status"]["reach"]["state"]
                .as_str()
                .unwrap()
                .to_string()
        }

        // A running record is pushed with the reach the kind claims.
        w.area.push_status(&w.entry).await.unwrap();
        assert_eq!(w.host.events.borrow().len(), 1);
        assert_eq!(reach_of(&w.host, 0), "reachable");
        assert_eq!(
            w.host.events.borrow()[0]["status"]["phase"],
            json!("running")
        );
        // A napping one is left to the poller.
        w.area.push_status(&nap).await.unwrap();
        assert_eq!(w.host.events.borrow().len(), 1);

        // status_now pushes either, a napping one with the reach its phase implies and no reason.
        w.area.status_now(&w.entry).await.unwrap();
        w.area.status_now(&nap).await.unwrap();
        assert_eq!(w.host.events.borrow().len(), 3);
        assert_eq!(reach_of(&w.host, 1), "reachable");
        assert_eq!(reach_of(&w.host, 2), "napping");
        let napping = w.host.events.borrow()[2].clone();
        assert_eq!(napping["status"]["id"], json!("ws_2"));
        assert_eq!(napping["status"]["machineState"], json!("paused"));
        assert!(napping["status"].get("reason").is_none());

        // A running record whose daemon has a line about it is pushed unreachable.
        knobs()
            .unreached
            .borrow_mut()
            .insert(WS.to_string(), "the daemon is down".to_string());
        w.area.status_now(&w.entry).await.unwrap();
        assert_eq!(reach_of(&w.host, 3), "unreachable");

        // One line about the wake in flight is kept on the entry and pushed with it.
        w.area.says_waking(&nap, "waking: 40 s in").await.unwrap();
        assert_eq!(
            nap.borrow().wake_said.clone(),
            Some("waking: 40 s in".to_string())
        );
        assert_eq!(w.host.events.borrow().len(), 5);
        let waking = w.host.events.borrow()[4].clone();
        assert_eq!(waking["status"]["reach"]["state"], json!("napping"));
        assert_eq!(waking["status"]["reason"], json!("waking: 40 s in"));
    });
}
