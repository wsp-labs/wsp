// SPDX-License-Identifier: AGPL-3.0-only
//! One test per scene of the create oracle: the same world, the same act (`createStaged` with the scene's options), and
//! the log, the answer and the live entry the TS produced. The fixture carries each scene's input as `scene`; the act is
//! encoded in `World::create` from oracle/create.oracle.ts. Spec: spec/create.md.
//!
//! Not asserted, because a `HostError` has no field for them: the `status` a cap refusal keeps and the `cause` a thrown
//! cap refusal carries (scenes at the machine cap).

mod common;
#[path = "support/create.rs"]
mod support;

use serde_json::{json, Value};
use support::{expected_log, kind_of, same, World, VIEW_MARK};

async fn replay(name: &str) {
    let fixture = common::fixture("create");
    let sc = common::scene(&fixture, name);
    let w = World::new(&sc["scene"]);
    let result = w.create().await;
    common::flush().await;

    let want = expected_log(sc);
    let got = w.log.take();
    for (i, (e, g)) in want.iter().zip(got.iter()).enumerate() {
        assert!(
            same(g, e),
            "scene {name:?}: log entry {i} differs\n got {g}\nwant {e}"
        );
    }
    assert_eq!(
        got.len(),
        want.len(),
        "scene {name:?}: log length differs; got {got:#?}"
    );

    let answer = &sc["answer"];
    match result {
        Ok(created) => {
            assert!(
                answer.get("threw").is_none(),
                "scene {name:?}: the create answered, the oracle threw {answer}"
            );
            assert_eq!(
                created.view.id,
                answer["id"].as_str().expect("id"),
                "scene {name:?}: the answer's id"
            );
            assert_eq!(
                created.view.name,
                answer["name"].as_str().expect("name"),
                "scene {name:?}: the answer's name"
            );
            assert_eq!(
                created.view.created_at, VIEW_MARK,
                "scene {name:?}: the answer is what ctx.view made"
            );
            assert_eq!(
                created.notice.as_deref(),
                answer.get("notice").and_then(Value::as_str),
                "scene {name:?}: the notice"
            );
        }
        Err(e) => {
            let threw = answer.get("threw").unwrap_or_else(|| {
                panic!("scene {name:?}: the create threw {e:?}, the oracle answered {answer}")
            });
            assert_eq!(
                e.message,
                threw.as_str().expect("a message"),
                "scene {name:?}: what was thrown"
            );
            assert_eq!(
                e.kind,
                kind_of(answer["kind"].as_str()),
                "scene {name:?}: the kind of what was thrown"
            );
        }
    }

    let want_entry = &sc["entry"];
    match (w.entry("ws_1"), want_entry.is_null()) {
        (None, true) => {}
        (Some(e), false) => {
            let e = e.borrow();
            assert_eq!(
                e.creating,
                want_entry["creating"] == json!(true),
                "scene {name:?}: creating"
            );
            assert_eq!(
                e.of_record_held,
                want_entry["ofRecordHeld"] == json!(true),
                "scene {name:?}: ofRecordHeld"
            );
            match want_entry["opensThread"].as_str() {
                None => assert_eq!(e.opens_thread, None, "scene {name:?}: opensThread"),
                Some(_) => {
                    let given = sc["scene"]["o"].get("opensThread").and_then(Value::as_str);
                    match given {
                        Some(t) => assert_eq!(
                            e.opens_thread.as_deref(),
                            Some(t),
                            "scene {name:?}: the thread the caller named"
                        ),
                        None => assert!(
                            e.opens_thread.as_deref().is_some_and(|t| !t.is_empty()),
                            "scene {name:?}: a minted thread id"
                        ),
                    }
                }
            }
        }
        (live, _) => panic!(
            "scene {name:?}: the live entry is {}, the oracle's is {want_entry}",
            if live.is_some() { "there" } else { "absent" }
        ),
    }
}

#[test]
fn a_provider_fork_lands_and_is_ready() {
    common::run_local(async { replay("a provider fork lands and is ready").await })
}

#[test]
fn a_size_not_offered_is_refused_before_any_machine() {
    common::run_local(async { replay("a size not offered is refused before any machine").await })
}

#[test]
fn an_offered_size_is_kept() {
    common::run_local(async { replay("an offered size is kept").await })
}

#[test]
fn the_road_refuses_before_anything_is_written() {
    common::run_local(async { replay("the road refuses before anything is written").await })
}

#[test]
fn a_memory_folder_the_road_reads_otherwise_is_remembered() {
    common::run_local(async {
        replay("a memory folder the road reads otherwise is remembered").await
    })
}

#[test]
fn a_project_with_a_checkout_is_copied_a_child_starts_from_its_base() {
    common::run_local(async {
        replay("a project with a checkout is copied, a child starts from its base").await
    })
}

#[test]
fn a_computer_that_keeps_no_image_forks_from_none() {
    common::run_local(async { replay("a computer that keeps no image forks from none").await })
}

#[test]
fn a_named_golden_at_a_place_is_copied_there_first() {
    common::run_local(async { replay("a named golden at a place is copied there first").await })
}

#[test]
fn a_spawned_fork_carries_its_tree_and_no_agents() {
    common::run_local(async { replay("a spawned fork carries its tree and no agents").await })
}

#[test]
fn a_brokered_fork_names_its_branch_and_the_thread_it_opens() {
    common::run_local(async {
        replay("a brokered fork names its branch and the thread it opens").await
    })
}

#[test]
fn a_brokered_fork_with_the_branch_named_keeps_it_held_and_opens_the_named_thread() {
    common::run_local(async {
        replay("a brokered fork with the branch named keeps it held and opens the named thread")
            .await
    })
}

#[test]
fn at_the_machine_cap_a_kept_builder_is_stopped_and_the_fork_goes() {
    common::run_local(async {
        replay("at the machine cap a kept builder is stopped and the fork goes").await
    })
}

#[test]
fn at_the_cap_with_builders_refused_each_time_the_refusal_names_the_holders() {
    common::run_local(async {
        replay("at the cap with builders refused each time the refusal names the holders").await
    })
}

#[test]
fn at_the_cap_with_nothing_to_stop_the_status_is_kept() {
    common::run_local(async { replay("at the cap with nothing to stop the status is kept").await })
}

#[test]
fn a_fork_that_fails_another_way_is_thrown_as_it_is() {
    common::run_local(async { replay("a fork that fails another way is thrown as it is").await })
}

#[test]
fn a_refusal_after_a_builder_stopped_that_is_not_the_cap_is_thrown() {
    common::run_local(async {
        replay("a refusal after a builder stopped that is not the cap is thrown").await
    })
}

#[test]
fn a_route_and_a_daemon_both_asked_the_sync_started() {
    common::run_local(async { replay("a route and a daemon: both asked, the sync started").await })
}

#[test]
fn a_route_that_is_not_minted_is_a_notice_the_daemon_answering() {
    common::run_local(async {
        replay("a route that is not minted is a notice, the daemon answering").await
    })
}

#[test]
fn a_daemon_that_does_not_answer_is_noted_and_the_memory_is_not_read() {
    common::run_local(async {
        replay("a daemon that does not answer is noted and the memory is not read").await
    })
}

#[test]
fn a_served_workspace_asks_neither() {
    common::run_local(async { replay("a served workspace asks neither").await })
}

#[test]
fn memory_read_to_another_size_says_so() {
    common::run_local(async { replay("memory read to another size says so").await })
}

#[test]
fn memory_read_to_another_size_with_a_size_named() {
    common::run_local(async { replay("memory read to another size with a size named").await })
}

#[test]
fn memory_at_a_place_is_not_read_here() {
    common::run_local(async { replay("memory at a place is not read here").await })
}

#[test]
fn a_machine_notice_joins_the_answer() {
    common::run_local(async { replay("a machine notice joins the answer").await })
}

#[test]
fn a_clone_that_fails_ends_the_create_before_it_is_persisted() {
    common::run_local(async {
        replay("a clone that fails ends the create before it is persisted").await
    })
}
