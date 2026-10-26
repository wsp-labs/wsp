// SPDX-License-Identifier: AGPL-3.0-only
//! One test per scene of the thread_ends oracle: the same world, the same act, and the log, the answers and the store
//! rows the TS wrote. The fixture does not carry the scripted exec answers or the act; each is encoded here from
//! oracle/thread_ends.oracle.ts. Spec: spec/thread_ends.md.

mod common;
#[path = "support/thread_ends.rs"]
mod support;

use serde_json::json;
use support::{finish, Answer, World};
use wsp_machines::machines::thread_ends::{EndAsk, PlaceEvent};

const NONE: EndAsk = EndAsk {
    remove: false,
    away: false,
};
const REMOVE: EndAsk = EndAsk {
    remove: true,
    away: false,
};
const AWAY: EndAsk = EndAsk {
    remove: false,
    away: true,
};
const AWAY_REMOVE: EndAsk = EndAsk {
    remove: true,
    away: true,
};

fn present() -> PlaceEvent {
    PlaceEvent::Present {
        place_id: "p1".into(),
    }
}

#[test]
fn remove_computer_away_owed_nothing_said() {
    common::run_local(async {
        support::end_scene("remove, computer away: owed, nothing said", vec![], REMOVE).await;
    })
}

#[test]
fn stop_computer_away_nothing_owed_nothing_said() {
    common::run_local(async {
        support::end_scene(
            "stop, computer away: nothing owed, nothing said",
            vec![],
            NONE,
        )
        .await;
    })
}

#[test]
fn away_no_link_owed_and_the_away_line() {
    common::run_local(async {
        support::end_scene("away, no link: owed and the away line", vec![], AWAY).await;
    })
}

#[test]
fn away_linked_owed_the_late_line_then_paid_in_the_background() {
    common::run_local(async {
        support::end_scene(
            "away, linked: owed, the late line, then paid in the background",
            vec![],
            AWAY_REMOVE,
        )
        .await;
    })
}

#[test]
fn away_linked_already_owed_not_owed_twice() {
    common::run_local(async {
        support::end_scene(
            "away, linked, already owed: not owed twice",
            vec![Answer::Throws],
            AWAY,
        )
        .await;
    })
}

#[test]
fn stop_linked_all_ended() {
    common::run_local(async {
        support::end_scene("stop, linked, all ended", vec![], NONE).await;
    })
}

#[test]
fn stop_linked_survivors_named() {
    common::run_local(async {
        support::end_scene(
            "stop, linked, survivors named",
            vec![Answer::Exit(
                1,
                "kill: 12\nprocesses 12 345 of thread-t1 still stand\n",
            )],
            NONE,
        )
        .await;
    })
}

#[test]
fn stop_linked_nonzero_with_no_pids_named() {
    common::run_local(async {
        support::end_scene(
            "stop, linked, nonzero with no pids named",
            vec![Answer::Exit(3, "something else")],
            NONE,
        )
        .await;
    })
}

#[test]
fn stop_linked_the_exec_throws_the_empty_left_line_nothing_cleared() {
    common::run_local(async {
        support::end_scene(
            "stop, linked, the exec throws: the empty left line, nothing cleared",
            vec![Answer::Throws],
            NONE,
        )
        .await;
    })
}

#[test]
fn remove_linked_all_ended_cleared() {
    common::run_local(async {
        support::end_scene("remove, linked, all ended: cleared", vec![], REMOVE).await;
    })
}

#[test]
fn remove_linked_survivors_owed_and_logged() {
    common::run_local(async {
        support::end_scene(
            "remove, linked, survivors: owed and logged",
            vec![Answer::Exit(1, "processes 7 of x")],
            REMOVE,
        )
        .await;
    })
}

#[test]
fn remove_linked_the_exec_throws_owed() {
    common::run_local(async {
        support::end_scene(
            "remove, linked, the exec throws: owed",
            vec![Answer::Throws],
            REMOVE,
        )
        .await;
    })
}

#[test]
fn owed_none_owed_answers_nothing() {
    common::run_local(async {
        let fixture = common::fixture("thread_ends");
        let scene = common::scene(&fixture, "owed: none owed answers nothing");
        let world = World::of_scene(scene, vec![]);
        let ends = world.ends();
        let owed = ends.owed("p1", "t1").await.expect("owed answers");
        world
            .log
            .push(json!(["answer", support::owed_answer(&owed)]));
        finish(&world, scene).await;
    })
}

#[test]
fn owed_then_paid_at_place_present() {
    common::run_local(async {
        let fixture = common::fixture("thread_ends");
        let scene = common::scene(&fixture, "owed then paid at place.present");
        let world = World::of_scene(
            scene,
            vec![Answer::Exit(0, ""), Answer::Exit(1, "processes 9 of y")],
        );
        let ends = world.ends();
        let owed = ends.owed("p1", "t1").await.expect("owed answers");
        world
            .log
            .push(json!(["answer", support::owed_answer(&owed)]));
        support::note_paid(&world, owed, "t1");
        ends.on_place_event(&present());
        common::flush().await;
        finish(&world, scene).await;
    })
}

#[test]
fn pay_keeps_a_busy_thread_and_one_whose_end_did_not_answer() {
    common::run_local(async {
        let fixture = common::fixture("thread_ends");
        let scene = common::scene(
            &fixture,
            "pay keeps a busy thread and one whose end did not answer",
        );
        let world = World::of_scene(scene, vec![Answer::Throws, Answer::Exit(0, "")]);
        let ends = world.ends();
        ends.on_place_event(&present());
        common::flush().await;
        finish(&world, scene).await;
    })
}

#[test]
fn place_removed_drops_the_row_and_settles_every_waiter_of_that_computer() {
    common::run_local(async {
        let fixture = common::fixture("thread_ends");
        let scene = common::scene(
            &fixture,
            "place.removed drops the row and settles every waiter of that computer",
        );
        let world = World::of_scene(scene, vec![]);
        let ends = world.ends();
        let a = ends.owed("p1", "t1").await.expect("owed answers");
        let b = ends.owed("p2", "t1").await.expect("owed answers");
        support::note_paid(&world, a, "p1");
        support::note_paid(&world, b, "p2");
        ends.on_place_event(&PlaceEvent::Removed {
            place_id: "p1".into(),
        });
        common::flush().await;
        finish(&world, scene).await;
    })
}

#[test]
fn pay_with_nothing_owed_asks_nothing() {
    common::run_local(async {
        let fixture = common::fixture("thread_ends");
        let scene = common::scene(&fixture, "pay with nothing owed asks nothing");
        let world = World::of_scene(scene, vec![]);
        let ends = world.ends();
        ends.on_place_event(&present());
        common::flush().await;
        finish(&world, scene).await;
    })
}

#[test]
fn other_events_are_ignored() {
    common::run_local(async {
        let fixture = common::fixture("thread_ends");
        let scene = common::scene(&fixture, "other events are ignored");
        let world = World::of_scene(scene, vec![]);
        let ends = world.ends();
        ends.on_place_event(&PlaceEvent::Other);
        common::flush().await;
        finish(&world, scene).await;
    })
}
