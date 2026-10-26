// SPDX-License-Identifier: AGPL-3.0-only
//! spec/reach_daemon.md: the wait on a woken daemon, the hello's version, the checkout read and a lead's tree, ported
//! from packages/runtime/test/box-nap.test.ts, wake-keeps-machine.test.ts, runtime-reach.test.ts, tree.test.ts and
//! changes.test.ts. The wait is driven through `ping_daemon`, the one public road to it.
mod machines_b_support;

use std::cell::Cell;
use std::rc::Rc;
use std::sync::Arc;

use serde_json::{json, Value};
use wsp_engine::Cancel;

use machines_b_support::reach_ctx::*;
use machines_b_support::*;
use wsp_engine::machine::MachineOffers;
use wsp_machines::machines::machines::Entry;
use wsp_machines::machines::reach::daemon::{LOOK_EXITS_EVERY_MS, START_DAEMON_AFTER_MS};
use wsp_machines::machines::reach::ReachArea;
use wsp_wire::wire::daemon::DETACHED_HEAD;
use wsp_words::words::computer::WAKE_STOPPED;

const WS: &str = "ws_1";
const REMOTE: &str = "https://github.com/wsp/stub-1.git";
/// types/events.ts `CHECKOUT_TTL_MS`.
const CHECKOUT_TTL_MS: f64 = 10_000.0;

struct W {
    host: Rc<FakeHost>,
    area: Rc<ReachArea>,
    entry: Entry,
    machine: Arc<FakeMachine>,
}

fn world_in(phase: &str, offers: MachineOffers) -> W {
    let host = reach_host();
    let machine = FakeMachine::new("bx_1");
    *machine.offers.lock().unwrap() = offers;
    let entry = host.put_live(plain_record(WS, "web", phase, "bx_1"), machine.clone());
    let area = reach(&host);
    W {
        host,
        area,
        entry,
        machine,
    }
}

/// A napped workspace whose wake is waiting on its machine.
fn world(offers: MachineOffers) -> W {
    world_in("napping", offers)
}

fn edge() -> MachineOffers {
    MachineOffers {
        preview_url: true,
        put_bytes: true,
        read_bytes: true,
        ..MachineOffers::default()
    }
}

fn answers_road() -> MachineOffers {
    MachineOffers {
        daemon_answers: true,
        put_bytes: true,
        read_bytes: true,
        ..MachineOffers::default()
    }
}

fn ping(w: &W) -> tokio::task::JoinHandle<Option<String>> {
    let (a, e) = (w.area.clone(), w.entry.clone());
    tokio::task::spawn_local(async move { a.ping_daemon(&e).await })
}

fn warns(w: &W) -> Vec<String> {
    w.host.warns.borrow().clone()
}

fn starts(w: &W) -> u32 {
    *w.machine.starts_asked.lock().unwrap()
}

fn late(secs: u32) -> String {
    format!("daemon on bx_1 (workspace {WS}) had not answered {secs} s into the wait, so wsp started it")
}

fn closed(i: usize) -> u32 {
    plan().links.borrow()[i].closed.get()
}

/// No timer of the wait's own (a start, a look) stands before `at`; the wait's cut may.
fn nothing_due_before(w: &W, at: f64) {
    let due = w.host.clock.due();
    assert!(
        due.iter().all(|d| *d >= at),
        "a timer stands before {at}: {due:?}"
    );
}

// ---------------------------------------------------------------------------------------------------------------
// The wait on a woken daemon.

#[test]
fn a_daemon_answering_late_within_the_budget_passes() {
    run(async {
        // The edge road: the box is up and its daemon answers the link two and a half minutes in (box-nap :227).
        let w = world(edge());
        let t0 = w.host.clock.now();
        let task = ping(&w);
        w.host.clock.advance(150_000.0).await;
        assert!(!task.is_finished());
        plan().set_up();
        settle().await;
        assert!(task.is_finished());
        assert_eq!(task.await.unwrap(), None);
        assert_eq!(w.host.clock.now() - t0, 150_000.0);
        nothing_due_before(&w, t0 + 900_000.0);
        let p = plan();
        assert_eq!(p.dials.borrow().len(), 1);
        assert_eq!(
            p.links.borrow()[0].requests.borrow().clone(),
            vec!["ping".to_string()]
        );
        assert_eq!(closed(0), 1);

        // The machine's own road is asked every half second until the budget is out, and the answer lands.
        let w = world(answers_road());
        {
            let mut q = w.machine.daemon_answers.lock().unwrap();
            for _ in 0..300 {
                q.push_back(Ok(false));
            }
            q.push_back(Ok(true));
        }
        let task = ping(&w);
        w.host.clock.advance(149_999.0).await;
        assert_eq!(w.machine.daemon_answers.lock().unwrap().len(), 1);
        assert!(!task.is_finished());
        w.host.clock.advance(1.0).await;
        assert_eq!(w.machine.daemon_answers.lock().unwrap().len(), 0);
        assert_eq!(task.await.unwrap(), None);
        assert!(plan().dials.borrow().is_empty());

        // Seven minutes in (box-nap :244): the same machine still passes inside the fifteen.
        let w = world(edge());
        let task = ping(&w);
        w.host.clock.advance(7.0 * MIN).await;
        plan().set_up();
        settle().await;
        assert_eq!(task.await.unwrap(), None);
    });
}

#[test]
fn a_daemon_never_started_is_started_a_minute_in() {
    run(async {
        let w = world(MachineOffers {
            start_daemon: true,
            ..edge()
        });
        let t0 = w.host.clock.now();
        let task = ping(&w);
        w.host.clock.advance(START_DAEMON_AFTER_MS - 1.0).await;
        assert_eq!(starts(&w), 0);
        assert!(warns(&w).is_empty());
        w.host.clock.advance(1.0).await;
        assert_eq!(starts(&w), 1);
        assert_eq!(warns(&w), vec![format!("{} (exit 0)", late(60))]);
        // The start's own answer does not end the wait: the daemon answering does.
        assert!(!task.is_finished());
        plan().set_up();
        settle().await;
        assert_eq!(task.await.unwrap(), None);
        nothing_due_before(&w, t0 + 900_000.0);
    });
}

#[test]
fn asks_for_the_start_again_a_minute_after_one_that_throws() {
    run(async {
        let w = world(MachineOffers {
            start_daemon: true,
            ..edge()
        });
        let t0 = w.host.clock.now();
        w.machine
            .start_daemon
            .lock()
            .unwrap()
            .push_back(Reply::Fail("Request timeout after 69998ms".to_string()));
        let task = ping(&w);
        w.host.clock.advance(START_DAEMON_AFTER_MS).await;
        assert_eq!(starts(&w), 1);
        assert_eq!(
            warns(&w),
            vec![format!(
                "{}, and the start failed (Request timeout after 69998ms)",
                late(60)
            )]
        );
        w.host.clock.advance(START_DAEMON_AFTER_MS - 1.0).await;
        assert_eq!(starts(&w), 1);
        w.host.clock.advance(1.0).await;
        assert_eq!(starts(&w), 2);
        assert_eq!(
            warns(&w),
            vec![
                format!(
                    "{}, and the start failed (Request timeout after 69998ms)",
                    late(60)
                ),
                format!("{} (exit 0)", late(120))
            ]
        );
        // A start that ran is not asked again; only the wait's own cut stands.
        nothing_due_before(&w, t0 + 900_000.0);
        w.host.clock.advance(3.0 * START_DAEMON_AFTER_MS).await;
        assert_eq!(starts(&w), 2);
        plan().set_up();
        settle().await;
        assert_eq!(task.await.unwrap(), None);
    });
}

#[test]
fn asks_for_the_start_again_a_minute_after_one_that_runs_out_its_timeout() {
    run(async {
        let w = world(MachineOffers {
            start_daemon: true,
            ..edge()
        });
        // Exit 124 is the exec deadline's own; the second answer is a failure with a long stderr, whose last 200
        // characters after a trim are what the line says.
        let stderr = format!("  {}END  \n", "a".repeat(300));
        let tail = format!("{}END", "a".repeat(197));
        {
            let mut q = w.machine.start_daemon.lock().unwrap();
            q.push_back(exit(124, ""));
            q.push_back(exit(1, &stderr));
        }
        let task = ping(&w);
        w.host.clock.advance(START_DAEMON_AFTER_MS).await;
        assert_eq!(warns(&w), vec![format!("{} (exit 124)", late(60))]);
        w.host.clock.advance(START_DAEMON_AFTER_MS).await;
        w.host.clock.advance(START_DAEMON_AFTER_MS).await;
        assert_eq!(starts(&w), 3);
        assert_eq!(
            warns(&w),
            vec![
                format!("{} (exit 124)", late(60)),
                format!("{} (exit 1: {tail})", late(120)),
                format!("{} (exit 0)", late(180))
            ]
        );
        w.host.clock.advance(2.0 * START_DAEMON_AFTER_MS).await;
        assert_eq!(starts(&w), 3);
        plan().set_up();
        settle().await;
        assert_eq!(task.await.unwrap(), None);
    });
}

#[test]
fn starts_nothing_on_a_machine_whose_daemon_answered() {
    run(async {
        let w = world(MachineOffers {
            start_daemon: true,
            ..edge()
        });
        let t0 = w.host.clock.now();
        plan().set_up();
        assert_eq!(ping(&w).await.unwrap(), None);
        nothing_due_before(&w, t0 + 900_000.0);
        // A start armed by the wait and never cleared would fire here.
        w.host.clock.advance(2.0 * START_DAEMON_AFTER_MS).await;
        assert_eq!(starts(&w), 0);
        assert!(warns(&w).is_empty());
    });
}

#[test]
fn a_daemon_that_never_answers_fails_when_the_budget_is_out() {
    run(async {
        // The edge road: the link never answers, so the fifteen minutes run out on it.
        let w = world(edge());
        let t0 = w.host.clock.now();
        let task = ping(&w);
        w.host.clock.advance(899_999.0).await;
        assert!(!task.is_finished());
        w.host.clock.advance(1.0).await;
        let fault = task.await.unwrap().expect("the wait failed");
        assert!(
            fault.starts_with(
                "daemon on bx_1 did not answer within 900000 ms (daemon link timed out after "
            ),
            "{fault}"
        );
        assert!(fault.ends_with(" ms)"), "{fault}");
        assert_eq!(w.host.clock.now() - t0, 900_000.0);
        assert_eq!(closed(0), 1);
        assert_eq!(w.host.clock.pending(), 0);

        // The machine's own road: asked until the budget is out, then the port's silence is the answer.
        let w = world(answers_road());
        knobs().daemon_answers_ms.set(Some(1_200.0));
        let task = ping(&w);
        w.host.clock.advance(1_499.0).await;
        assert!(!task.is_finished());
        w.host.clock.advance(1.0).await;
        assert_eq!(
            task.await.unwrap(),
            Some("nothing listens on the daemon's port inside bx_1".to_string())
        );

        // An error from the road itself is what the row says.
        let w = world(answers_road());
        w.machine
            .daemon_answers
            .lock()
            .unwrap()
            .push_back(Err("boom".to_string()));
        assert_eq!(
            ping(&w).await.unwrap(),
            Some("the daemon on bx_1 could not be asked (boom)".to_string())
        );

        // A machine with neither road has nothing to ask.
        let w = world(MachineOffers::default());
        assert_eq!(ping(&w).await.unwrap(), None);
        assert!(plan().dials.borrow().is_empty());
        assert_eq!(w.host.clock.pending(), 0);
    });
}

#[test]
fn a_daemon_that_exits_as_it_starts_fails_with_its_own_words() {
    run(async {
        // A box whose binary predates --doors, under today's unit, which Restart=always starts again every second.
        let w = world(MachineOffers {
            daemon_exits: true,
            ..edge()
        });
        let words = "error: unexpected argument '--doors' found; Usage: wsp-daemon [OPTIONS]";
        {
            let mut q = w.machine.daemon_exits.lock().unwrap();
            q.push_back(Ok(None));
            // A look the machine could not answer is asked again.
            q.push_back(Err("Request timeout after 5000ms".to_string()));
            q.push_back(Ok(Some(words.to_string())));
        }
        let t0 = w.host.clock.now();
        let task = ping(&w);
        w.host.clock.advance(3.0 * LOOK_EXITS_EVERY_MS - 1.0).await;
        assert!(!task.is_finished());
        w.host.clock.advance(1.0).await;
        assert_eq!(
            task.await.unwrap(),
            Some(format!("the daemon on bx_1 exits as it starts ({words})"))
        );
        assert_eq!(w.host.clock.now() - t0, 3.0 * LOOK_EXITS_EVERY_MS);
        assert_eq!(w.machine.daemon_exits.lock().unwrap().len(), 0);
        assert_eq!(closed(0), 1);
        // A look armed by the wait and never cleared would ask again here.
        w.machine.daemon_exits.lock().unwrap().push_back(Ok(None));
        w.host.clock.advance(12.0 * LOOK_EXITS_EVERY_MS).await;
        assert_eq!(w.machine.daemon_exits.lock().unwrap().len(), 1);
        nothing_due_before(&w, t0 + 900_000.0);
    });
}

#[test]
fn a_stop_pulled_during_the_wait_lets_go_at_once() {
    run(async {
        let w = world(edge());
        let stop = Cancel::new();
        w.entry.borrow_mut().wake_stop = Some(stop.clone());
        let t0 = w.host.clock.now();
        let task = ping(&w);
        w.host.clock.advance(10_000.0).await;
        assert!(!task.is_finished());
        stop.cancel(None);
        settle().await;
        assert!(task.is_finished());
        assert_eq!(
            task.await.unwrap(),
            Some(format!(
                "daemon on bx_1 did not answer within 900000 ms ({WAKE_STOPPED})"
            ))
        );
        assert_eq!(w.host.clock.now() - t0, 10_000.0);
        assert_eq!(closed(0), 1);
        nothing_due_before(&w, t0 + 900_000.0);

        // A stop already pulled still makes the first ask, whose answer is thrown away (reach.ts:240).
        let w = world(answers_road());
        w.machine.daemon_answers.lock().unwrap().push_back(Ok(true));
        let stop = Cancel::new();
        stop.cancel(None);
        w.entry.borrow_mut().wake_stop = Some(stop.clone());
        assert_eq!(
            ping(&w).await.unwrap(),
            Some(format!(
                "the daemon on bx_1 could not be asked ({WAKE_STOPPED})"
            ))
        );
        assert_eq!(w.machine.daemon_answers.lock().unwrap().len(), 0);
    });
}

// reach.ts:207 `connectDaemon(..)` throws on options it cannot dial; the edge road's catch (reach.ts:268) says it with the budget.
#[test]
fn a_link_the_runtime_cannot_open_is_the_edge_road_not_answering() {
    run(async {
        let w = world(edge());
        plan().set_up();
        *knobs().connect_failure.borrow_mut() = Some(wsp_words::exit::HostError::new(
            "connectDaemon needs the daemon's token to dial a previewUrl",
        ));
        assert_eq!(
            ping(&w).await.unwrap(),
            Some("daemon on bx_1 did not answer within 900000 ms (connectDaemon needs the daemon's token to dial a previewUrl)".to_string())
        );
        assert!(plan().links.borrow().is_empty(), "no link was made");
    });
}

#[test]
fn the_edge_road_pings_through_the_link_and_closes_it() {
    run(async {
        // The dial carries the preview route, this runtime's token and the budget as the link's heartbeat.
        let w = world(edge());
        knobs().daemon_answers_ms.set(Some(300.0));
        plan().set_up();
        assert_eq!(ping(&w).await.unwrap(), None);
        let p = plan();
        assert_eq!(
            p.dials.borrow().clone(),
            vec![Dial {
                preview_url: "http://127.0.0.1:1/".to_string(),
                token: "tok-1".to_string(),
                heartbeat_ms: Some(300.0)
            }]
        );
        assert_eq!(
            p.links.borrow()[0].requests.borrow().clone(),
            vec!["ping".to_string()]
        );
        assert_eq!(closed(0), 1);
        assert_eq!(
            w.host.calls_of("daemon_token_of"),
            vec![vec!["bx_1".to_string()]]
        );

        // A ping the daemon refuses is the fault, and the link is still let go.
        let w = world(edge());
        knobs().daemon_answers_ms.set(Some(300.0));
        plan().set_up();
        *plan().ping_error.borrow_mut() = Some("edge 502".to_string());
        assert_eq!(
            ping(&w).await.unwrap(),
            Some("daemon on bx_1 did not answer within 300 ms (edge 502)".to_string())
        );
        assert_eq!(closed(0), 1);

        // A guest with no daemon token is asked a command instead: an exec that returns is its own answer.
        let w = world(edge());
        *knobs().token.borrow_mut() = None;
        assert_eq!(ping(&w).await.unwrap(), None);
        assert_eq!(w.machine.log(), vec!["true".to_string()]);
        assert!(plan().dials.borrow().is_empty());

        // And an exec that fails is the fault.
        let w = world(edge());
        *knobs().token.borrow_mut() = None;
        knobs().daemon_answers_ms.set(Some(300.0));
        w.machine
            .answer("true", Reply::Fail("guest down".to_string()));
        assert_eq!(
            ping(&w).await.unwrap(),
            Some("daemon on bx_1 did not answer within 300 ms (guest down)".to_string())
        );
    });
}

// ---------------------------------------------------------------------------------------------------------------
// The hello.

#[test]
fn hello_version_reads_the_hello_and_closes_the_link() {
    run(async {
        let w = world(edge());
        let t0 = w.host.clock.now();
        // Two hellos: the first frame's version is the answer.
        plan().hellos.borrow_mut().extend([7.0, 9.0]);
        assert_eq!(w.area.hello_version(&w.entry).await, Some(7.0));
        let p = plan();
        assert_eq!(
            p.dials.borrow().clone(),
            vec![Dial {
                preview_url: "http://127.0.0.1:1/".to_string(),
                token: "tok-1".to_string(),
                heartbeat_ms: None
            }]
        );
        assert_eq!(closed(0), 1);
        nothing_due_before(&w, t0 + 2_000.0);
    });
}

#[test]
fn hello_version_is_none_without_a_route_or_a_token() {
    run(async {
        // A backend that mints no preview route: nothing is asked of the guest at all.
        let w = world(MachineOffers::default());
        plan().hellos.borrow_mut().push(7.0);
        assert_eq!(w.area.hello_version(&w.entry).await, None);
        assert!(plan().dials.borrow().is_empty());
        assert!(w.host.calls_of("daemon_token_of").is_empty());

        // A guest with no token has no daemon to dial.
        let w = world(edge());
        *knobs().token.borrow_mut() = None;
        plan().hellos.borrow_mut().push(7.0);
        assert_eq!(w.area.hello_version(&w.entry).await, None);
        assert_eq!(
            w.host.calls_of("daemon_token_of"),
            vec![vec!["bx_1".to_string()]]
        );
        assert!(plan().dials.borrow().is_empty());

        // A daemon that says nothing inside the bound has none either, and its link is let go.
        let w = world(edge());
        let (a, e) = (w.area.clone(), w.entry.clone());
        let task = tokio::task::spawn_local(async move { a.hello_version(&e).await });
        w.host.clock.advance(1_999.0).await;
        assert!(!task.is_finished());
        w.host.clock.advance(1.0).await;
        assert_eq!(task.await.unwrap(), None);
        assert_eq!(closed(0), 1);
        assert_eq!(w.host.clock.pending(), 0);
    });
}

// ---------------------------------------------------------------------------------------------------------------
// The checkout read.

fn running(offers: MachineOffers) -> W {
    world_in("running", offers)
}

fn fact(entry: &Entry) -> Value {
    serde_json::to_value(entry.borrow().checkout.clone()).unwrap()
}

fn asked(w: &W) -> usize {
    w.host.calls_of("with_daemon").len()
}

fn set_phase(entry: &Entry, phase: &str) {
    entry.borrow().record.borrow_mut().phase = serde_json::from_value(json!(phase)).unwrap();
}

#[test]
fn read_checkout_answers_the_held_fact_within_the_ttl_and_reads_when_forced() {
    run(async {
        let w = running(MachineOffers::default());
        knobs().has_daemon.set(true);
        answer_status(WS, status_reply("fix/cart", "abc", &[".M", "??", "!!"]));
        let t0 = w.host.clock.now();

        // The first read: the daemon's reply as the fact, ignored files not counted, the frame as the TS sends it.
        let got = w
            .area
            .read_checkout(&w.entry, false)
            .await
            .expect("a fact was read");
        let want =
            json!({ "branch": "fix/cart", "ahead": 1, "behind": 0, "changed": 2, "readAt": t0 });
        assert_eq!(
            js_text(&serde_json::to_value(&got).unwrap()),
            js_text(&want)
        );
        assert_eq!(js_text(&fact(&w.entry)), js_text(&want));
        assert_eq!(
            w.host.calls_of("with_daemon"),
            vec![vec![
                WS.to_string(),
                js_text(&json!({ "op": "git.status", "cwd": CWD }))
            ]]
        );
        // It rode the workspace's status, and was written on the record.
        let seen = w.host.events_of("workspace.status");
        assert_eq!(seen.len(), 1);
        assert_eq!(seen[0]["status"]["checkout"]["branch"], json!("fix/cart"));
        assert_eq!(seen[0]["status"]["reach"]["state"], json!("reachable"));
        assert_eq!(
            js_text(&w.host.stored(WS).unwrap()["checkout"]),
            js_text(&want)
        );

        // Younger than the window: the fact held answers. At exactly the window it is read again.
        w.host.clock.advance(CHECKOUT_TTL_MS - 1.0).await;
        let held = w.area.read_checkout(&w.entry, false).await.unwrap();
        assert_eq!(
            canon(&serde_json::to_value(&held).unwrap()["readAt"]),
            canon(&json!(t0))
        );
        assert_eq!(asked(&w), 1);
        w.host.clock.advance(1.0).await;
        w.area.read_checkout(&w.entry, false).await.unwrap();
        assert_eq!(asked(&w), 2);
        assert_eq!(
            canon(&fact(&w.entry)["readAt"]),
            canon(&json!(t0 + CHECKOUT_TTL_MS))
        );
        // Forced: read again inside the window.
        w.area.read_checkout(&w.entry, true).await.unwrap();
        assert_eq!(asked(&w), 3);

        // A stopped copy is asked nothing, forced or not, unless its computer serves it.
        set_phase(&w.entry, "napping");
        w.host.clock.advance(2.0 * CHECKOUT_TTL_MS).await;
        let kept = fact(&w.entry);
        assert_eq!(
            js_text(&serde_json::to_value(w.area.read_checkout(&w.entry, false).await).unwrap()),
            js_text(&kept)
        );
        assert_eq!(
            js_text(&serde_json::to_value(w.area.read_checkout(&w.entry, true).await).unwrap()),
            js_text(&kept)
        );
        assert_eq!(asked(&w), 3);
        knobs().served.set(true);
        w.area.read_checkout(&w.entry, false).await.unwrap();
        assert_eq!(asked(&w), 4);
        // Its status goes out with the reach the phase implies.
        assert_eq!(
            w.host.events_of("workspace.status").last().unwrap()["status"]["reach"]["state"],
            json!("napping")
        );

        // A copy git could not read keeps the last fact, the time on it saying how old it is.
        let events = w.host.events_of("workspace.status").len();
        let kept = fact(&w.entry);
        fail_status(WS, "git gone");
        w.host.clock.advance(2.0 * CHECKOUT_TTL_MS).await;
        let after = w
            .area
            .read_checkout(&w.entry, false)
            .await
            .expect("the last fact stands");
        assert_eq!(
            js_text(&serde_json::to_value(&after).unwrap()),
            js_text(&kept)
        );
        assert_eq!(js_text(&fact(&w.entry)), js_text(&kept));
        assert_eq!(asked(&w), 5);
        assert_eq!(w.host.events_of("workspace.status").len(), events);

        // What the daemon says rides the fact in its key order: unread edits, unknown counts, stashes, the head's commit.
        let w = running(MachineOffers::default());
        let oid = "0123456789abcdef0123456789abcdef01234567";
        let mut reply = status_reply("main", oid, &[".M", "!!", "??", "!!"]);
        reply["editsUnread"] = json!(true);
        reply["countsUnknown"] = json!(true);
        reply["stashes"] = json!(2);
        answer_status(WS, reply);
        let t1 = w.host.clock.now();
        let got = w.area.read_checkout(&w.entry, true).await.unwrap();
        let want = json!({ "branch": "main", "ahead": 1, "behind": 0, "changed": 2, "editsUnread": true, "countsUnknown": true, "stashes": 2, "head": oid, "readAt": t1 });
        assert_eq!(
            js_text(&serde_json::to_value(&got).unwrap()),
            js_text(&want)
        );
        // Only a lower case hash of 7 to 40 characters is a head: git has no commit yet, or the shape is another's.
        let forty = "a".repeat(40);
        let long = "a".repeat(41);
        for (oid, has_head) in [
            ("abcdef1", true),
            (forty.as_str(), true),
            ("abcdef", false),
            (long.as_str(), false),
            ("ABCDEF1", false),
            ("(initial)", false),
        ] {
            answer_status(WS, status_reply("main", oid, &[]));
            w.area.read_checkout(&w.entry, true).await.unwrap();
            assert_eq!(fact(&w.entry).get("head").is_some(), has_head, "oid {oid}");
            if has_head {
                assert_eq!(fact(&w.entry)["head"], json!(oid));
            }
            assert!(
                fact(&w.entry).get("stashes").is_none()
                    && fact(&w.entry).get("editsUnread").is_none(),
                "oid {oid}"
            );
        }
    });
}

#[test]
fn read_checkout_writes_the_record_only_when_the_fact_moved() {
    run(async {
        let w = running(MachineOffers::default());
        answer_status(WS, status_reply("fix/cart", "abc", &[".M"]));
        let t0 = w.host.clock.now();
        let record_fact = |w: &W| {
            serde_json::to_value(w.entry.borrow().record.borrow().checkout.clone()).unwrap()
        };

        // The first fact differs from the record's none: written and persisted.
        w.area.read_checkout(&w.entry, true).await.unwrap();
        assert_eq!(w.host.calls_of("persist"), vec![vec![WS.to_string()]]);
        assert_eq!(
            js_text(&record_fact(&w)),
            js_text(
                &json!({ "branch": "fix/cart", "ahead": 1, "behind": 0, "changed": 1, "readAt": t0 })
            )
        );
        assert_eq!(
            js_text(&w.host.stored(WS).unwrap()["checkout"]),
            js_text(&record_fact(&w))
        );

        // The same facts a half minute later: the entry's fact is newer, the record is not written again, the status still goes.
        w.host.clock.advance(30_000.0).await;
        w.area.read_checkout(&w.entry, false).await.unwrap();
        assert_eq!(
            canon(&fact(&w.entry)["readAt"]),
            canon(&json!(t0 + 30_000.0))
        );
        assert_eq!(canon(&record_fact(&w)["readAt"]), canon(&json!(t0)));
        assert_eq!(w.host.calls_of("persist").len(), 1);
        assert_eq!(w.host.events_of("workspace.status").len(), 2);
        assert_eq!(
            canon(&w.host.events_of("workspace.status")[1]["status"]["checkout"]["readAt"]),
            canon(&json!(t0 + 30_000.0))
        );

        // One more changed file: the record takes the fact, time and all, and is persisted.
        answer_status(WS, status_reply("fix/cart", "abc", &[".M", ".M"]));
        w.area.read_checkout(&w.entry, true).await.unwrap();
        assert_eq!(w.host.calls_of("persist").len(), 2);
        assert_eq!(canon(&record_fact(&w)["changed"]), canon(&json!(2)));
        assert_eq!(
            canon(&record_fact(&w)["readAt"]),
            canon(&json!(t0 + 30_000.0))
        );
        assert_eq!(
            canon(&w.host.stored(WS).unwrap()["checkout"]["changed"]),
            canon(&json!(2))
        );
        assert_eq!(w.host.events_of("workspace.status").len(), 3);
    });
}

#[test]
fn read_checkout_joins_a_read_in_flight() {
    run(async {
        let w = running(MachineOffers::default());
        answer_status(WS, status_reply("fix/cart", "abc", &[".M"]));
        let held = gate();
        *knobs().status_gate.borrow_mut() = Some(held.clone());
        let (a, e) = (w.area.clone(), w.entry.clone());
        let first = tokio::task::spawn_local(async move { a.read_checkout(&e, false).await });
        settle().await;
        assert_eq!(asked(&w), 1);
        assert!(w.entry.borrow().checkout_reading.is_some());
        // A second caller, forced, joins the one in flight rather than asking the daemon again.
        let (a, e) = (w.area.clone(), w.entry.clone());
        let second = tokio::task::spawn_local(async move { a.read_checkout(&e, true).await });
        settle().await;
        assert_eq!(asked(&w), 1);
        assert!(!first.is_finished() && !second.is_finished());

        open(&held);
        settle().await;
        let (one, two) = (
            first.await.unwrap().expect("a fact"),
            second.await.unwrap().expect("a fact"),
        );
        assert_eq!(
            js_text(&serde_json::to_value(&one).unwrap()),
            js_text(&serde_json::to_value(&two).unwrap())
        );
        assert_eq!(
            js_text(&serde_json::to_value(&one).unwrap()),
            js_text(&fact(&w.entry))
        );
        assert_eq!(asked(&w), 1);
        assert_eq!(w.host.calls_of("persist").len(), 1);
        assert_eq!(w.host.events_of("workspace.status").len(), 1);
        // The slot is cleared when the read settles, so the next forced read asks again.
        assert!(w.entry.borrow().checkout_reading.is_none());
        w.area.read_checkout(&w.entry, true).await.unwrap();
        assert_eq!(asked(&w), 2);
    });
}

// ---------------------------------------------------------------------------------------------------------------
// The lead's tree.

struct T {
    host: Rc<FakeHost>,
    area: Rc<ReachArea>,
    lead: Entry,
}

fn lead_record(base: Option<&str>) -> Value {
    let mut r = plain_record("ws_lead", "lead", "running", "bx_lead");
    r["project"] = json!("pr_lead");
    if let Some(b) = base {
        r["base"] = json!(b);
    }
    r
}

fn tree_world(lead_base: Option<&str>) -> T {
    let host = reach_host();
    set_project("pr_lead", "", "main", None);
    set_project("pr_kid", REMOTE, "main", None);
    set_project("pr_nohost", "", "main", None);
    let lead = host.put_live(lead_record(lead_base), FakeMachine::new("bx_lead"));
    let area = reach(&host);
    T { host, area, lead }
}

fn put_kid(host: &FakeHost, id: &str, project: &str, extra: Value) -> Entry {
    let mut r = plain_record(id, id, "running", &format!("m_{id}"));
    r["parentWorkspaceId"] = json!("ws_lead");
    r["project"] = json!(project);
    if let Value::Object(more) = extra {
        for (k, v) in more {
            r[k.as_str()] = v;
        }
    }
    host.put_live(r, FakeMachine::new(&format!("m_{id}")))
}

fn on_branch(id: &str, head: &str) {
    answer_status(id, status_reply(head, "abc", &[]));
}

fn compare_frame(base: &str, head: &str) -> String {
    js_text(
        &json!({ "op": "git.branchCompare", "cwd": HOME, "remote": REMOTE, "base": base, "head": head }),
    )
}

fn compared(t: &T) -> Vec<String> {
    let mut got: Vec<String> = t
        .host
        .calls_of("on_this_computer")
        .into_iter()
        .map(|a| a[0].clone())
        .collect();
    got.sort();
    got
}

fn tree_of(lead: &Entry) -> Value {
    serde_json::to_value(lead.borrow().tree.clone()).unwrap()
}

#[test]
fn reads_each_childs_branch_against_the_leads() {
    run(async {
        let t = tree_world(None);
        let host = &t.host;
        on_branch("ws_lead", "tree/lead");
        let t0 = host.clock.now();
        put_kid(host, "ws_c1", "pr_kid", json!({}));
        on_branch("ws_c1", "child/one");
        put_kid(host, "ws_c2", "pr_kid", json!({}));
        on_branch("ws_c2", "child/two");
        // A child still being made, and one that belongs to another lead, are not this lead's children.
        let making = put_kid(host, "ws_new", "pr_kid", json!({}));
        making.borrow_mut().creating = true;
        put_kid(
            host,
            "ws_other",
            "pr_kid",
            json!({ "parentWorkspaceId": "ws_elsewhere" }),
        );
        // On no branch, on a project whose remote names no host, and a compare nothing could answer: no counts.
        put_kid(host, "ws_c3", "pr_kid", json!({}));
        on_branch("ws_c3", DETACHED_HEAD);
        put_kid(host, "ws_c4", "pr_nohost", json!({}));
        on_branch("ws_c4", "child/four");
        put_kid(host, "ws_c5", "pr_kid", json!({}));
        on_branch("ws_c5", "child/five");
        // The thread is the first row on the child that has one.
        put_session(host, "s_a", "ws_c1", None, None, None);
        put_session(host, "s_b", "ws_c1", Some("t_c1"), None, None);
        put_session(host, "s_c", "ws_c1", Some("t_c1b"), None, None);
        on_compare(|f| match f["head"].as_str() {
            Some("child/one") => {
                Ok(json!({ "pushed": true, "aheadBy": 2, "behindBy": 0, "status": "ahead" }))
            }
            Some("child/five") => Err("compare failed".to_string()),
            _ => Ok(json!({ "pushed": false })),
        });

        let tree = t
            .area
            .read_tree(&t.lead)
            .await
            .expect("the lead has children");
        let want = json!({
            "leadBranch": "tree/lead",
            "children": [
                { "workspaceId": "ws_c1", "threadId": "t_c1", "branch": "child/one", "pushed": true, "aheadOfLead": 2, "behindLead": 0 },
                { "workspaceId": "ws_c2", "branch": "child/two", "pushed": false },
                { "workspaceId": "ws_c3", "branch": DETACHED_HEAD },
                { "workspaceId": "ws_c4", "branch": "child/four" },
                { "workspaceId": "ws_c5", "branch": "child/five" }
            ],
            "readAt": t0
        });
        assert_eq!(
            js_text(&serde_json::to_value(&tree).unwrap()),
            js_text(&want)
        );
        assert_eq!(js_text(&tree_of(&t.lead)), js_text(&want));
        assert!(t.lead.borrow().tree_reading.is_none());
        // Only the lead's own children were read, once each; the compare ran for the three branches with a host.
        let mut read: Vec<String> = host
            .calls_of("with_daemon")
            .into_iter()
            .map(|a| a[0].clone())
            .collect();
        read.sort();
        assert_eq!(
            read,
            ["ws_c1", "ws_c2", "ws_c3", "ws_c4", "ws_c5", "ws_lead"]
                .map(String::from)
                .to_vec()
        );
        let mut frames: Vec<String> = ["child/one", "child/two", "child/five"]
            .iter()
            .map(|h| compare_frame("tree/lead", h))
            .collect();
        frames.sort();
        assert_eq!(compared(&t), frames);
        // The tree rode the lead's status.
        let on_lead: Vec<Value> = host
            .events_of("workspace.status")
            .into_iter()
            .filter(|e| e["status"]["id"] == json!("ws_lead"))
            .collect();
        assert_eq!(
            js_text(&on_lead.last().unwrap()["status"]["tree"]),
            js_text(&want)
        );

        // A child's own read of its lead, never on a timer: it reads the tree again where the lead is live.
        host.clock.advance(1_000.0).await;
        let c1 = host
            .live
            .borrow()
            .get("ws_c1")
            .cloned()
            .expect("a live child");
        t.area.read_lead_of(&c1);
        settle().await;
        assert_eq!(
            canon(&tree_of(&t.lead)["readAt"]),
            canon(&json!(t0 + 1_000.0))
        );
        assert_eq!(compared(&t).len(), 6);
        // A child whose lead is not live reads nothing.
        let orphan = put_kid(
            host,
            "ws_orphan",
            "pr_kid",
            json!({ "parentWorkspaceId": "ws_gone" }),
        );
        t.area.read_lead_of(&orphan);
        settle().await;
        assert_eq!(compared(&t).len(), 6);
        assert_eq!(
            canon(&tree_of(&t.lead)["readAt"]),
            canon(&json!(t0 + 1_000.0))
        );

        // A lead whose own branch could not be read falls back to its record's base, then its project's base, then the
        // project's default branch. A child whose branch could not be read falls back to its worktree's, then its base.
        let cases = [
            (Some("release"), None, "release"),
            (None, Some("dev"), "dev"),
            (None, None, "main"),
        ];
        for (lead_base, project_base, leads) in cases {
            let t = tree_world(lead_base);
            set_project("pr_lead", "", "main", project_base);
            fail_status("ws_lead", "no daemon");
            put_kid(&t.host, "ws_c1", "pr_kid", json!({}));
            on_branch("ws_c1", "child/one");
            put_kid(
                &t.host,
                "ws_c2",
                "pr_kid",
                json!({ "worktree": { "path": "/wt/x", "made": true, "branch": "wt/x" } }),
            );
            fail_status("ws_c2", "no daemon");
            put_kid(&t.host, "ws_c3", "pr_kid", json!({ "base": "feat" }));
            fail_status("ws_c3", "no daemon");
            on_compare(|_| Ok(json!({ "pushed": false })));
            let tree = t.area.read_tree(&t.lead).await.expect("a tree");
            let want = json!({
                "leadBranch": leads,
                "children": [
                    { "workspaceId": "ws_c1", "branch": "child/one", "pushed": false },
                    { "workspaceId": "ws_c2", "branch": "wt/x", "pushed": false },
                    { "workspaceId": "ws_c3", "branch": "feat", "pushed": false }
                ],
                "readAt": t.host.clock.now()
            });
            assert_eq!(
                js_text(&serde_json::to_value(&tree).unwrap()),
                js_text(&want),
                "lead base {lead_base:?}, project base {project_base:?}"
            );
            let mut frames: Vec<String> = ["child/one", "wt/x", "feat"]
                .iter()
                .map(|h| compare_frame(leads, h))
                .collect();
            frames.sort();
            assert_eq!(compared(&t), frames);
        }
    });
}

#[test]
fn a_child_merged_counts_against_the_commit_the_merge_took() {
    run(async {
        let t = tree_world(None);
        let since = Rc::new(Cell::new(0i64));
        let s = since.clone();
        // The remote's lead branch lacks the merge until the lead's bring back; the commit the merge took is in the copy.
        on_compare(move |f| {
            if f["base"] == json!("c0de") {
                let status = if s.get() > 0 { "ahead" } else { "identical" };
                Ok(json!({ "pushed": true, "aheadBy": s.get(), "behindBy": 0, "status": status }))
            } else {
                Ok(json!({ "pushed": true, "aheadBy": 2, "behindBy": 0, "status": "ahead" }))
            }
        });
        let merged_tree = json!({ "merged": { "oid": "d00d", "at": 1, "head": "c0de" }, "conflicts": ["lead.txt"], "pushRefused": "no credential here" });
        on_branch("ws_lead", "tree/lead");
        put_kid(&t.host, "ws_c1", "pr_kid", json!({ "tree": merged_tree }));
        on_branch("ws_c1", "child/one");

        // Nothing ahead at once, counted against the commit the merge took; the record's keys ride last.
        let tree = t.area.read_tree(&t.lead).await.expect("a tree");
        let row = |tree: &Value| canon(&tree["children"][0]);
        let want = |ahead: i64| {
            canon(&json!({
                "workspaceId": "ws_c1", "branch": "child/one", "pushed": true, "aheadOfLead": ahead, "behindLead": 0,
                "merged": { "oid": "d00d", "at": 1, "head": "c0de" }, "conflicts": ["lead.txt"], "pushRefused": "no credential here"
            }))
        };
        assert_eq!(row(&serde_json::to_value(&tree).unwrap()), want(0));
        assert_eq!(compared(&t), vec![compare_frame("c0de", "child/one")]);
        // Its new commits read as ahead again.
        since.set(1);
        t.area.read_tree(&t.lead).await.expect("a tree");
        assert_eq!(row(&tree_of(&t.lead)), want(1));

        // A second reader of a tree in flight joins it.
        let t = tree_world(None);
        on_branch("ws_lead", "tree/lead");
        put_kid(&t.host, "ws_c1", "pr_kid", json!({}));
        on_branch("ws_c1", "child/one");
        let held = gate();
        *knobs().status_gate.borrow_mut() = Some(held.clone());
        let (a, l) = (t.area.clone(), t.lead.clone());
        let first = tokio::task::spawn_local(async move { a.read_tree(&l).await });
        settle().await;
        let (a, l) = (t.area.clone(), t.lead.clone());
        let second = tokio::task::spawn_local(async move { a.read_tree(&l).await });
        settle().await;
        assert_eq!(t.host.calls_of("with_daemon").len(), 1);
        assert!(t.lead.borrow().tree_reading.is_some());
        open(&held);
        settle().await;
        let (one, two) = (
            first.await.unwrap().expect("a tree"),
            second.await.unwrap().expect("a tree"),
        );
        assert_eq!(
            js_text(&serde_json::to_value(&one).unwrap()),
            js_text(&serde_json::to_value(&two).unwrap())
        );
        let mut read: Vec<String> = t
            .host
            .calls_of("with_daemon")
            .into_iter()
            .map(|a| a[0].clone())
            .collect();
        read.sort();
        assert_eq!(read, vec!["ws_c1".to_string(), "ws_lead".to_string()]);
        assert!(t.lead.borrow().tree_reading.is_none());
    });
}

#[test]
fn a_lead_whose_children_are_gone_loses_its_tree() {
    run(async {
        let t = tree_world(None);
        t.lead.borrow_mut().tree = Some(
            serde_json::from_value(
                json!({ "leadBranch": "tree/lead", "children": [], "readAt": 1 }),
            )
            .unwrap(),
        );
        // The only child is still being made, which is no child yet.
        let making = put_kid(&t.host, "ws_c1", "pr_kid", json!({}));
        making.borrow_mut().creating = true;

        assert_eq!(
            t.area.read_tree(&t.lead).await.expect("the read answers"),
            None
        );
        assert!(t.lead.borrow().tree.is_none());
        let seen = t.host.events_of("workspace.status");
        assert_eq!(seen.len(), 1);
        assert_eq!(seen[0]["status"]["id"], json!("ws_lead"));
        assert!(seen[0]["status"].get("tree").is_none());
        assert!(t.host.calls_of("with_daemon").is_empty());

        // Nothing held and nothing to read: no status goes out.
        assert_eq!(
            t.area.read_tree(&t.lead).await.expect("the read answers"),
            None
        );
        assert_eq!(t.host.events_of("workspace.status").len(), 1);

        // The child lands, is read, then goes: the tree read of it is taken off.
        making.borrow_mut().creating = false;
        on_branch("ws_lead", "tree/lead");
        on_branch("ws_c1", "child/one");
        on_compare(|_| Ok(json!({ "pushed": false })));
        let tree = t.area.read_tree(&t.lead).await.expect("a tree");
        assert_eq!(
            serde_json::to_value(&tree).unwrap()["children"][0]["workspaceId"],
            json!("ws_c1")
        );
        assert!(t.lead.borrow().tree.is_some());
        let pushed = t.host.events_of("workspace.status").len();
        t.host.live.borrow_mut().shift_remove("ws_c1");
        assert_eq!(
            t.area.read_tree(&t.lead).await.expect("the read answers"),
            None
        );
        assert!(t.lead.borrow().tree.is_none());
        let seen = t.host.events_of("workspace.status");
        assert_eq!(seen.len(), pushed + 1);
        assert!(seen.last().unwrap()["status"].get("tree").is_none());
    });
}
