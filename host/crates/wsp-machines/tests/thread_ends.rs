// SPDX-License-Identifier: AGPL-3.0-only
//! The rules of `machines/thread_ends.rs` the oracle scenes do not name, and the two box-suite cases written against
//! the fakes. Spec: spec/thread_ends.md.

mod common;
#[path = "support/thread_ends.rs"]
mod support;

use serde_json::{json, Value};
use support::{Answer, World};
use wsp_machines::machines::thread_ends::{survivors_of, EndAsk, PlaceEvent, OWED};

const AWAY_REMOVE: EndAsk = EndAsk {
    remove: true,
    away: true,
};
const REMOVE: EndAsk = EndAsk {
    remove: true,
    away: false,
};
const STOP: EndAsk = EndAsk {
    remove: false,
    away: false,
};

fn present(place: &str) -> PlaceEvent {
    PlaceEvent::Present {
        place_id: place.into(),
    }
}

/// The script the oracle's scene `name` logged for its `n`th `door.exec`.
fn script_of(scene: &str, n: usize) -> String {
    let fixture = common::fixture("thread_ends");
    common::scene(&fixture, scene)["log"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|e| e[0] == "door.exec")
        .nth(n)
        .unwrap_or_else(|| panic!("scene {scene:?} has no exec {n}"))[2]
        .as_str()
        .unwrap()
        .to_string()
}

/// The words the oracle's scene `name` answered with.
fn answer_of(scene: &str) -> String {
    let fixture = common::fixture("thread_ends");
    common::scene(&fixture, scene)["log"]
        .as_array()
        .unwrap()
        .iter()
        .find(|e| e[0] == "answer")
        .unwrap_or_else(|| panic!("scene {scene:?} has no answer"))[1]
        .as_str()
        .unwrap()
        .to_string()
}

fn owed_row(threads: &[&str]) -> Value {
    json!([["p1", { "threads": threads }]])
}

#[test]
fn survivors_of_reads_the_pids_the_end_script_names() {
    assert_eq!(survivors_of("processes 12 345 of x"), vec![12, 345]);
    assert_eq!(survivors_of("processes 7 of x"), vec![7]);
    assert_eq!(
        survivors_of("kill: 12\nprocesses 12 345 of thread-t1 still stand\n"),
        vec![12, 345]
    );
    assert_eq!(
        survivors_of("leading text, then processes 4 5 6 of /wsp-threads/t1 did not end"),
        vec![4, 5, 6]
    );
    assert_eq!(
        survivors_of("processes 1 2 of a\nprocesses 3 of b"),
        vec![1, 2]
    );
    assert_eq!(survivors_of(""), Vec::<i64>::new());
    assert_eq!(survivors_of("something else"), Vec::<i64>::new());
    assert_eq!(survivors_of("processes of x"), Vec::<i64>::new());
}

#[test]
fn owe_appends_once_and_an_emptied_list_deletes_the_row() {
    common::run_local(async {
        let world = World::new(
            &json!({}),
            &json!({ "linked": [], "running": [], "launching": [], "holds": [] }),
            vec![],
        );
        let ends = world.ends();
        let store = || world.log_of(&["store.get", "store.put", "store.delete"]);

        ends.end("p1", "t1", REMOVE).await.unwrap();
        assert_eq!(
            store(),
            vec![
                json!(["store.get", OWED, "p1"]),
                json!(["store.put", OWED, "p1", { "threads": ["t1"] }])
            ]
        );
        assert_eq!(world.store.table(OWED), owed_row(&["t1"]));

        world.log.clear();
        ends.end("p1", "t1", REMOVE).await.unwrap();
        assert_eq!(
            store(),
            vec![json!(["store.get", OWED, "p1"])],
            "a second debt of one thread writes nothing"
        );
        assert_eq!(world.store.table(OWED), owed_row(&["t1"]));

        world.log.clear();
        ends.end("p1", "t2", REMOVE).await.unwrap();
        assert_eq!(
            store(),
            vec![
                json!(["store.get", OWED, "p1"]),
                json!(["store.put", OWED, "p1", { "threads": ["t1", "t2"] }])
            ]
        );
        assert_eq!(world.store.table(OWED), owed_row(&["t1", "t2"]));

        world.linked.borrow_mut().insert("p1".into());
        world.log.clear();
        assert_eq!(ends.end("p1", "t1", STOP).await.unwrap(), None);
        assert_eq!(
            store(),
            vec![
                json!(["store.get", OWED, "p1"]),
                json!(["store.put", OWED, "p1", { "threads": ["t2"] }])
            ]
        );
        assert_eq!(world.store.table(OWED), owed_row(&["t2"]));

        world.log.clear();
        assert_eq!(ends.end("p1", "t2", STOP).await.unwrap(), None);
        assert_eq!(
            store(),
            vec![
                json!(["store.get", OWED, "p1"]),
                json!(["store.delete", OWED, "p1"])
            ],
            "an emptied list deletes the row"
        );
        assert_eq!(world.store.table(OWED), json!([]));
    })
}

#[test]
fn owed_registers_its_waiter_before_the_read() {
    common::run_local(async {
        let world = World::new(
            &json!({ "p1": { "threads": ["t1"] } }),
            &json!({ "linked": ["p1"], "running": [], "launching": [], "holds": [] }),
            vec![Answer::Exit(0, "")],
        );
        let ends = world.ends();
        let release = world.hold_next_get();
        let asking = tokio::task::spawn_local({
            let ends = ends.clone();
            async move { ends.owed("p1", "t1").await }
        });
        common::flush().await;
        assert!(!asking.is_finished(), "the read is held");

        // The payment lands while the read is in flight: it ends t1 on p1 and takes the row.
        ends.on_place_event(&present("p1"));
        common::flush().await;
        assert_eq!(
            world.store.table(OWED),
            json!([]),
            "the payment cleared the row"
        );
        let paid_entries = world.log_of(&["door.exec"]);
        assert_eq!(paid_entries.len(), 1);
        assert_eq!(
            paid_entries[0][2],
            script_of("owed then paid at place.present", 0)
        );

        release.send(()).unwrap();
        let owed = asking
            .await
            .unwrap()
            .expect("owed answers")
            .expect("the read saw the debt, so it is owed");
        assert!(
            futures::future::poll_immediate(owed.paid).await.is_some(),
            "the payment that landed during the read settled the waiter"
        );
    })
}

#[test]
fn a_stop_on_an_away_box_owes_the_end_and_a_present_pays_it() {
    common::run_local(async {
        let world = World::new(
            &json!({}),
            &json!({ "linked": [], "running": [], "launching": [], "holds": [] }),
            vec![],
        );
        let ends = world.ends();
        let said = ends.end("p1", "t1", AWAY_REMOVE).await.unwrap();
        assert_eq!(
            said,
            Some(answer_of("away, no link: owed and the away line"))
        );
        assert_eq!(world.store.table(OWED), owed_row(&["t1"]));
        assert!(
            world.log_of(&["door.exec"]).is_empty(),
            "nothing is asked of an away box"
        );

        world.linked.borrow_mut().insert("p1".into());
        ends.on_place_event(&present("p1"));
        common::flush().await;
        let execs = world.log_of(&["door.exec"]);
        assert_eq!(
            execs,
            vec![
                json!(["door.exec", "p1", script_of("away, linked: owed, the late line, then paid in the background", 0), { "timeoutMs": 20000 }])
            ]
        );
        assert_eq!(
            world.store.table(OWED),
            json!([]),
            "the row is deleted once the end ran"
        );
    })
}

#[test]
fn a_send_after_a_restart_waits_until_the_owed_end_ran() {
    common::run_local(async {
        // After a restart the debt is only the row: a fresh area over the same store, no waiters.
        let world = World::new(
            &json!({ "p1": { "threads": ["t1"] } }),
            &json!({ "linked": [], "running": [], "launching": [], "holds": [] }),
            vec![],
        );
        let ends = world.ends();
        let owed = ends.owed("p1", "t1").await.unwrap();
        assert!(owed.is_some(), "the row still owes t1");
        world.log.clear();
        support::note_paid(&world, owed, "t1");
        common::flush().await;
        assert!(
            world.log_of(&["paid"]).is_empty(),
            "the send waits while the box is away"
        );

        world.linked.borrow_mut().insert("p1".into());
        ends.on_place_event(&present("p1"));
        common::flush().await;
        assert_eq!(
            world.log_of(&["door.exec", "store.delete", "paid"]),
            vec![
                json!(["door.exec", "p1", script_of("away, linked: owed, the late line, then paid in the background", 0), { "timeoutMs": 20000 }]),
                json!(["store.delete", OWED, "p1"]),
                json!(["paid", "t1"]),
            ],
            "paid resolves only after the end ran and the row went"
        );
        assert_eq!(world.store.table(OWED), json!([]));
    })
}
