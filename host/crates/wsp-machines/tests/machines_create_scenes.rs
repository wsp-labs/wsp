// SPDX-License-Identifier: AGPL-3.0-only
//! One test per scene of "machines: creates" in oracle/machines.oracle.ts: the same world, the same act, and the log,
//! the answer, the `creates` rows and `inflight` the TS left (tests/fixtures/oracle/machines_creates.json). The fixture
//! carries the inputs and the expected result, not the act; each act is encoded here from the oracle. Spec:
//! spec/machines_create.md.

mod common;
#[path = "support/machines.rs"]
mod support;

use std::sync::Arc;

use serde_json::{json, Value};
use support::{from, BackendOpts, MachineDef, Made, Opts, Replay, World};
use wsp_engine::machine::{MachineBackend, MachineState, NamedMachine};
use wsp_words::exit::HostError;

#[test]
fn a_keyed_create_puts_its_attempt_creates_under_its_key_and_drops_the_attempt() {
    common::run_local(async {
        let fx = common::fixture("machines_creates");
        let scene = common::scene(
            &fx,
            "a keyed create puts its attempt, creates under its key and drops the attempt",
        );
        let w = World::new(Opts {
            store: scene["store"].clone(),
            ..Default::default()
        });
        let b = w.provider(BackendOpts::default());
        let answer = support::answer_of(
            support::create(&w, b, "workspace/ws_1", support::spec_default(), None).await,
        );
        common::flush().await;
        support::check_create(&w, scene, answer);
    })
}

#[test]
fn a_create_with_a_request_reads_it_first_and_keeps_it_on_the_attempt() {
    common::run_local(async {
        let fx = common::fixture("machines_creates");
        let scene = common::scene(
            &fx,
            "a create with a request reads it first and keeps it on the attempt",
        );
        let w = World::new(Opts {
            store: scene["store"].clone(),
            ..Default::default()
        });
        let b = w.provider(BackendOpts {
            request: true,
            ..Default::default()
        });
        let answer = support::answer_of(
            support::create(
                &w,
                b,
                "workspace/ws_1",
                support::spec_default(),
                Some("pl1"),
            )
            .await,
        );
        common::flush().await;
        support::check_create(&w, scene, answer);
    })
}

#[test]
fn a_held_attempt_of_a_dead_pid_of_this_host_with_the_same_body_is_replayed_under_i() {
    common::run_local(async {
        let fx = common::fixture("machines_creates");
        let scene = common::scene(&fx, "a held attempt of a dead pid of this host with the same body is replayed under its key and stamp");
        let w = World::new(Opts {
            store: support::store_with_fingerprint(scene),
            ..Default::default()
        });
        let b = w.provider(BackendOpts::default());
        let answer = support::answer_of(
            support::create(&w, b, "workspace/ws_1", support::spec_default(), None).await,
        );
        common::flush().await;
        support::check_create(&w, scene, answer);
    })
}

#[test]
fn a_held_attempt_of_this_very_process_is_replayed() {
    common::run_local(async {
        let fx = common::fixture("machines_creates");
        let scene = common::scene(&fx, "a held attempt of this very process is replayed");
        let w = World::new(Opts {
            store: support::store_with_fingerprint(scene),
            ..Default::default()
        });
        let b = w.provider(BackendOpts::default());
        let answer = support::answer_of(
            support::create(&w, b, "workspace/ws_1", support::spec_default(), None).await,
        );
        common::flush().await;
        support::check_create(&w, scene, answer);
    })
}

#[test]
fn an_attempt_another_live_process_holds_is_never_joined() {
    common::run_local(async {
        let fx = common::fixture("machines_creates");
        let scene = common::scene(&fx, "an attempt another live process holds is never joined");
        let w = World::new(Opts {
            store: support::store_with_fingerprint(scene),
            ..Default::default()
        });
        let b = w.provider(BackendOpts::default());
        let answer = support::answer_of(
            support::create(&w, b, "workspace/ws_1", support::spec_default(), None).await,
        );
        common::flush().await;
        support::check_create(&w, scene, answer);
    })
}

#[test]
fn an_attempt_of_another_host_is_never_joined() {
    common::run_local(async {
        let fx = common::fixture("machines_creates");
        let scene = common::scene(&fx, "an attempt of another host is never joined");
        let w = World::new(Opts {
            store: support::store_with_fingerprint(scene),
            ..Default::default()
        });
        let b = w.provider(BackendOpts::default());
        let answer = support::answer_of(
            support::create(&w, b, "workspace/ws_1", support::spec_default(), None).await,
        );
        common::flush().await;
        support::check_create(&w, scene, answer);
    })
}

#[test]
fn a_changed_request_starts_a_new_attempt() {
    common::run_local(async {
        let fx = common::fixture("machines_creates");
        let scene = common::scene(&fx, "a changed request starts a new attempt");
        let w = World::new(Opts {
            store: scene["store"].clone(),
            ..Default::default()
        });
        let b = w.provider(BackendOpts::default());
        let answer = support::answer_of(
            support::create(&w, b, "workspace/ws_1", support::spec_default(), None).await,
        );
        common::flush().await;
        support::check_create(&w, scene, answer);
    })
}

#[test]
fn an_attempt_held_from_before_its_bytes_were_kept_is_never_replayed_where_the_back() {
    common::run_local(async {
        let fx = common::fixture("machines_creates");
        let scene = common::scene(&fx, "an attempt held from before its bytes were kept is never replayed where the backend sends bytes");
        let w = World::new(Opts {
            store: support::store_with_fingerprint(scene),
            ..Default::default()
        });
        let b = w.provider(BackendOpts {
            request: true,
            ..Default::default()
        });
        let answer = support::answer_of(
            support::create(&w, b, "workspace/ws_1", support::spec_default(), None).await,
        );
        common::flush().await;
        support::check_create(&w, scene, answer);
    })
}

#[test]
fn a_refusal_with_a_status_ends_the_attempt() {
    common::run_local(async {
        let fx = common::fixture("machines_creates");
        let scene = common::scene(&fx, "a refusal with a status ends the attempt");
        let w = World::new(Opts {
            store: scene["store"].clone(),
            ..Default::default()
        });
        let b = w.provider(BackendOpts {
            creates: vec![Made::Fail(support::status_err(409, "conflict"))],
            ..Default::default()
        });
        let answer = {
            let mut a = support::answer_of(
                support::create(&w, b, "workspace/ws_1", support::spec_default(), None).await,
            );
            a["kind"] = Value::Null;
            a
        };
        common::flush().await;
        support::check_create(&w, scene, answer);
    })
}

#[test]
fn a_failure_with_no_status_keeps_the_attempt_for_a_retry() {
    common::run_local(async {
        let fx = common::fixture("machines_creates");
        let scene = common::scene(
            &fx,
            "a failure with no status keeps the attempt for a retry",
        );
        let w = World::new(Opts {
            store: scene["store"].clone(),
            ..Default::default()
        });
        let b = w.provider(BackendOpts {
            creates: vec![Made::Fail(support::plain("socket hang up"))],
            ..Default::default()
        });
        let answer = support::answer_of(
            support::create(&w, b, "workspace/ws_1", support::spec_default(), None).await,
        );
        common::flush().await;
        support::check_create(&w, scene, answer);
    })
}

#[test]
fn a_replay_naming_a_gone_machine_is_created_anew_once() {
    common::run_local(async {
        let fx = common::fixture("machines_creates");
        let scene = common::scene(&fx, "a replay naming a gone machine is created anew once");
        let w = World::new(Opts {
            store: scene["store"].clone(),
            ..Default::default()
        });
        let b = w.provider(BackendOpts {
            creates: vec![
                Made::Machine(MachineDef::new("m-corpse").replayed(MachineState::Gone)),
                Made::Machine(MachineDef::new("m-fresh")),
            ],
            ..Default::default()
        });
        let answer = support::answer_of(
            support::create(&w, b, "workspace/ws_1", support::spec_default(), None).await,
        );
        common::flush().await;
        support::check_create(&w, scene, answer);
    })
}

#[test]
fn a_second_gone_replay_is_thrown() {
    common::run_local(async {
        let fx = common::fixture("machines_creates");
        let scene = common::scene(&fx, "a second gone replay is thrown");
        let w = World::new(Opts {
            store: scene["store"].clone(),
            ..Default::default()
        });
        let b = w.provider(BackendOpts {
            creates: vec![
                Made::Machine(MachineDef::new("m-corpse").replayed(MachineState::Gone)),
                Made::Machine(MachineDef::new("m-corpse2").replayed(MachineState::Gone)),
            ],
            ..Default::default()
        });
        let answer = support::answer_of(
            support::create(&w, b, "workspace/ws_1", support::spec_default(), None).await,
        );
        common::flush().await;
        support::check_create(&w, scene, answer);
    })
}

#[test]
fn a_replay_of_a_live_machine_is_said_and_kept() {
    common::run_local(async {
        let fx = common::fixture("machines_creates");
        let scene = common::scene(&fx, "a replay of a live machine is said and kept");
        let w = World::new(Opts {
            store: scene["store"].clone(),
            ..Default::default()
        });
        let b = w.provider(BackendOpts {
            creates: vec![Made::Machine(
                MachineDef::new("m-again").replayed(MachineState::Running),
            )],
            ..Default::default()
        });
        let answer = support::answer_of(
            support::create(&w, b, "workspace/ws_1", support::spec_default(), None).await,
        );
        common::flush().await;
        support::check_create(&w, scene, answer);
    })
}

#[test]
fn a_purpose_past_the_key_s_room_is_hashed() {
    common::run_local(async {
        let fx = common::fixture("machines_creates");
        let scene = common::scene(&fx, "a purpose past the key's room is hashed");
        let w = World::new(Opts {
            store: scene["store"].clone(),
            ..Default::default()
        });
        let b = w.provider(BackendOpts::default());
        let answer = support::answer_of(
            support::create(
                &w,
                b,
                &format!("workspace/{}", "x".repeat(240)),
                support::spec_default(),
                None,
            )
            .await,
        );
        common::flush().await;
        support::check_create(&w, scene, answer);
    })
}

#[test]
fn a_purpose_exactly_at_the_key_s_room_is_kept() {
    common::run_local(async {
        let fx = common::fixture("machines_creates");
        let scene = common::scene(&fx, "a purpose exactly at the key's room is kept");
        let w = World::new(Opts {
            store: scene["store"].clone(),
            ..Default::default()
        });
        let b = w.provider(BackendOpts::default());
        let answer = support::answer_of(
            support::create(
                &w,
                b,
                &format!("w/{}", "x".repeat(236)),
                support::spec_default(),
                None,
            )
            .await,
        );
        common::flush().await;
        support::check_create(&w, scene, answer);
    })
}

#[test]
fn a_spec_with_no_created_at_label_is_sent_as_it_is() {
    common::run_local(async {
        let fx = common::fixture("machines_creates");
        let scene = common::scene(&fx, "a spec with no created-at label is sent as it is");
        let w = World::new(Opts {
            store: scene["store"].clone(),
            ..Default::default()
        });
        let b = w.provider(BackendOpts::default());
        let answer = support::answer_of(
            support::create(
                &w,
                b,
                "workspace/ws_1",
                from(
                    json!({ "kind": "sandbox", "fromSnapshot": "snap-9", "cpu": 1, "memMb": 2048 }),
                ),
                None,
            )
            .await,
        );
        common::flush().await;
        support::check_create(&w, scene, answer);
    })
}

#[test]
fn replay_a_lost_attempt_s_machine_no_record_waits_for_is_stopped_and_reported_lost() {
    common::run_local(async {
        let fx = common::fixture("machines_creates");
        let scene = common::scene(
            &fx,
            "replay: a lost attempt's machine no record waits for is stopped and reported lost",
        );
        let w = World::new(Opts {
            store: support::store_of(scene),
            backend: Some(BackendOpts {
                replays: Some(vec![Replay::Machine(
                    MachineDef::new("m-lost").labels(&[("w", "1")]),
                )]),
                ..Default::default()
            }),
            ..Default::default()
        });
        let known: Vec<String> = vec![];
        let mut failed = Vec::new();
        let deleted = w
            .area
            .replay_creates(&|| known.clone(), &mut failed)
            .await
            .expect("the sweep answers");
        common::flush().await;
        let answer = json!({ "deleted": deleted.iter().map(support::reaped_json).collect::<Vec<_>>(), "failed": failed.iter().map(support::failure_json).collect::<Vec<_>>() });
        support::check_create(&w, scene, answer);
    })
}

#[test]
fn replay_rows_it_may_not_touch_are_skipped() {
    common::run_local(async {
        let fx = common::fixture("machines_creates");
        let scene = common::scene(&fx, "replay: rows it may not touch are skipped");
        let w = World::new(Opts {
            store: support::store_of(scene),
            backend: Some(BackendOpts {
                replays: Some(vec![]),
                ..Default::default()
            }),
            ..Default::default()
        });
        let known: Vec<String> = vec![];
        let mut failed = Vec::new();
        let deleted = w
            .area
            .replay_creates(&|| known.clone(), &mut failed)
            .await
            .expect("the sweep answers");
        common::flush().await;
        let answer = json!({ "deleted": deleted.iter().map(support::reaped_json).collect::<Vec<_>>(), "failed": failed.iter().map(support::failure_json).collect::<Vec<_>>() });
        support::check_create(&w, scene, answer);
    })
}

#[test]
fn replay_a_row_at_its_place_is_asked_there() {
    common::run_local(async {
        let fx = common::fixture("machines_creates");
        let scene = common::scene(&fx, "replay: a row at its place is asked there");
        let w = World::new(Opts {
            store: support::store_of(scene),
            backend: Some(BackendOpts {
                replays: Some(vec![Replay::Machine(MachineDef::new("m-lost"))]),
                ..Default::default()
            }),
            ..Default::default()
        });
        let known: Vec<String> = vec![];
        let mut failed = Vec::new();
        let deleted = w
            .area
            .replay_creates(&|| known.clone(), &mut failed)
            .await
            .expect("the sweep answers");
        common::flush().await;
        let answer = json!({ "deleted": deleted.iter().map(support::reaped_json).collect::<Vec<_>>(), "failed": failed.iter().map(support::failure_json).collect::<Vec<_>>() });
        support::check_create(&w, scene, answer);
    })
}

#[test]
fn replay_a_backend_with_no_replay_keeps_the_row() {
    common::run_local(async {
        let fx = common::fixture("machines_creates");
        let scene = common::scene(&fx, "replay: a backend with no replay keeps the row");
        let w = World::new(Opts {
            store: support::store_of(scene),
            backend: Some(BackendOpts::default()),
            ..Default::default()
        });
        let known: Vec<String> = vec![];
        let mut failed = Vec::new();
        let deleted = w
            .area
            .replay_creates(&|| known.clone(), &mut failed)
            .await
            .expect("the sweep answers");
        common::flush().await;
        let answer = json!({ "deleted": deleted.iter().map(support::reaped_json).collect::<Vec<_>>(), "failed": failed.iter().map(support::failure_json).collect::<Vec<_>>() });
        support::check_create(&w, scene, answer);
    })
}

#[test]
fn replay_missing_drops_the_row_another_failure_keeps_it_and_says_so() {
    common::run_local(async {
        let fx = common::fixture("machines_creates");
        let scene = common::scene(
            &fx,
            "replay: missing drops the row, another failure keeps it and says so",
        );
        let w = World::new(Opts {
            store: support::store_of(scene),
            backend: Some(BackendOpts {
                replays: Some(vec![
                    Replay::Fail(support::missing("404 not found")),
                    Replay::Fail(support::plain("timeout")),
                ]),
                ..Default::default()
            }),
            ..Default::default()
        });
        let known: Vec<String> = vec![];
        let mut failed = Vec::new();
        let deleted = w
            .area
            .replay_creates(&|| known.clone(), &mut failed)
            .await
            .expect("the sweep answers");
        common::flush().await;
        let answer = json!({ "deleted": deleted.iter().map(support::reaped_json).collect::<Vec<_>>(), "failed": failed.iter().map(support::failure_json).collect::<Vec<_>>() });
        support::check_create(&w, scene, answer);
    })
}

#[test]
fn replay_past_the_key_s_window_the_row_goes_and_the_box_is_the_stray_sweep_s() {
    common::run_local(async {
        let fx = common::fixture("machines_creates");
        let scene = common::scene(
            &fx,
            "replay: past the key's window the row goes and the box is the stray sweep's",
        );
        let w = World::new(Opts {
            store: support::store_of(scene),
            backend: Some(BackendOpts {
                replays: Some(vec![Replay::Window]),
                ..Default::default()
            }),
            ..Default::default()
        });
        let known: Vec<String> = vec![];
        let mut failed = Vec::new();
        let deleted = w
            .area
            .replay_creates(&|| known.clone(), &mut failed)
            .await
            .expect("the sweep answers");
        common::flush().await;
        let answer = json!({ "deleted": deleted.iter().map(support::reaped_json).collect::<Vec<_>>(), "failed": failed.iter().map(support::failure_json).collect::<Vec<_>>() });
        support::check_create(&w, scene, answer);
    })
}

#[test]
fn replay_a_known_machine_or_one_already_gone_is_only_forgotten() {
    common::run_local(async {
        let fx = common::fixture("machines_creates");
        let scene = common::scene(
            &fx,
            "replay: a known machine or one already gone is only forgotten",
        );
        let w = World::new(Opts {
            store: support::store_of(scene),
            backend: Some(BackendOpts {
                replays: Some(vec![
                    Replay::Machine(MachineDef::new("m-known")),
                    Replay::Machine(MachineDef::new("m-gone").seen(MachineState::Gone)),
                ]),
                ..Default::default()
            }),
            ..Default::default()
        });
        let known: Vec<String> = vec!["m-known".to_string()];
        let mut failed = Vec::new();
        let deleted = w
            .area
            .replay_creates(&|| known.clone(), &mut failed)
            .await
            .expect("the sweep answers");
        common::flush().await;
        let answer = json!({ "deleted": deleted.iter().map(support::reaped_json).collect::<Vec<_>>(), "failed": failed.iter().map(support::failure_json).collect::<Vec<_>>() });
        support::check_create(&w, scene, answer);
    })
}

#[test]
fn replay_a_kept_name_whose_project_is_held_waits_for_the_sweep_to_record_it() {
    common::run_local(async {
        let fx = common::fixture("machines_creates");
        let scene = common::scene(
            &fx,
            "replay: a kept name whose project is held waits for the sweep to record it",
        );
        let w = World::new(Opts {
            store: support::store_of(scene),
            backend: Some(BackendOpts {
                replays: Some(vec![Replay::Machine(MachineDef::new("m-lost"))]),
                ..Default::default()
            }),
            ..Default::default()
        });
        let known: Vec<String> = vec![];
        let mut failed = Vec::new();
        let deleted = w
            .area
            .replay_creates(&|| known.clone(), &mut failed)
            .await
            .expect("the sweep answers");
        common::flush().await;
        let answer = json!({ "deleted": deleted.iter().map(support::reaped_json).collect::<Vec<_>>(), "failed": failed.iter().map(support::failure_json).collect::<Vec<_>>() });
        support::check_create(&w, scene, answer);
    })
}

#[test]
fn replay_a_kept_name_whose_project_is_gone_is_stopped() {
    common::run_local(async {
        let fx = common::fixture("machines_creates");
        let scene = common::scene(&fx, "replay: a kept name whose project is gone is stopped");
        let w = World::new(Opts {
            store: support::store_of(scene),
            backend: Some(BackendOpts {
                replays: Some(vec![Replay::Machine(MachineDef::new("m-lost"))]),
                ..Default::default()
            }),
            ..Default::default()
        });
        let known: Vec<String> = vec![];
        let mut failed = Vec::new();
        let deleted = w
            .area
            .replay_creates(&|| known.clone(), &mut failed)
            .await
            .expect("the sweep answers");
        common::flush().await;
        let answer = json!({ "deleted": deleted.iter().map(support::reaped_json).collect::<Vec<_>>(), "failed": failed.iter().map(support::failure_json).collect::<Vec<_>>() });
        support::check_create(&w, scene, answer);
    })
}

#[test]
fn replay_a_stop_that_fails_keeps_the_row() {
    common::run_local(async {
        let fx = common::fixture("machines_creates");
        let scene = common::scene(&fx, "replay: a stop that fails keeps the row");
        let w = World::new(Opts {
            store: support::store_of(scene),
            backend: Some(BackendOpts {
                replays: Some(vec![Replay::Machine(MachineDef::new("m-lost"))]),
                ..Default::default()
            }),
            ..Default::default()
        });
        w.rt.cfg.borrow_mut().gone_stop_err = Some(HostError::new("busy"));
        let known: Vec<String> = vec![];
        let mut failed = Vec::new();
        let deleted = w
            .area
            .replay_creates(&|| known.clone(), &mut failed)
            .await
            .expect("the sweep answers");
        common::flush().await;
        let answer = json!({ "deleted": deleted.iter().map(support::reaped_json).collect::<Vec<_>>(), "failed": failed.iter().map(support::failure_json).collect::<Vec<_>>() });
        support::check_create(&w, scene, answer);
    })
}

#[test]
fn replay_a_stop_that_finds_it_missing_reports_it() {
    common::run_local(async {
        let fx = common::fixture("machines_creates");
        let scene = common::scene(&fx, "replay: a stop that finds it missing reports it");
        let w = World::new(Opts {
            store: support::store_of(scene),
            backend: Some(BackendOpts {
                replays: Some(vec![Replay::Machine(MachineDef::new("m-lost"))]),
                ..Default::default()
            }),
            ..Default::default()
        });
        w.rt.cfg.borrow_mut().gone_stop_err = Some(support::missing_host("404 not found"));
        let known: Vec<String> = vec![];
        let mut failed = Vec::new();
        let deleted = w
            .area
            .replay_creates(&|| known.clone(), &mut failed)
            .await
            .expect("the sweep answers");
        common::flush().await;
        let answer = json!({ "deleted": deleted.iter().map(support::reaped_json).collect::<Vec<_>>(), "failed": failed.iter().map(support::failure_json).collect::<Vec<_>>() });
        support::check_create(&w, scene, answer);
    })
}

#[test]
fn strays_only_this_host_s_image_names_never_known_or_asked() {
    common::run_local(async {
        let fx = common::fixture("machines_creates");
        let scene = common::scene(
            &fx,
            "strays: only this host's image names, never known or asked",
        );
        let w = World::new(Opts {
            store: support::store_of(scene),
            ..Default::default()
        });
        w.rt.cfg.borrow_mut().recorded_images = vec![
            "wsp-abc1h0a993e4c-kept".into(),
            "wsp-otherhost-x".into(),
            "plain".into(),
        ];
        let named =
            |id: &str, name: &str, created_at: Option<String>, state: MachineState| NamedMachine {
                id: id.into(),
                name: name.into(),
                state,
                created_at,
            };
        let b = w.provider(BackendOpts {
            named: Some(vec![
                named(
                    "s-old",
                    "wsp-abc1h0a993e4c-kept",
                    Some(support::before(600_000)),
                    MachineState::Running,
                ),
                named(
                    "s-young",
                    "wsp-abc1h0a993e4c-kept",
                    Some(support::before(599_999)),
                    MachineState::Running,
                ),
                named(
                    "s-known",
                    "wsp-abc1h0a993e4c-kept",
                    Some(support::before(120_000)),
                    MachineState::Running,
                ),
                named(
                    "s-asked",
                    "wsp-abc1h0a993e4c-asked",
                    Some(support::before(900_000)),
                    MachineState::Running,
                ),
                named(
                    "s-gone",
                    "wsp-abc1h0a993e4c-kept",
                    Some(support::before(900_000)),
                    MachineState::Gone,
                ),
                named(
                    "s-nodate",
                    "wsp-abc1h0a993e4c-kept",
                    None,
                    MachineState::Running,
                ),
            ]),
            ..Default::default()
        });
        let plain = w.provider(BackendOpts::default());
        let mut failed = Vec::new();
        w.log.push(json!(["mark", w.area.image_mark()]));
        let known = vec!["s-known".to_string()];
        let deleted = w
            .area
            .sweep_strays(
                &|| known.clone(),
                &mut failed,
                &[
                    b as Arc<dyn MachineBackend>,
                    plain as Arc<dyn MachineBackend>,
                ],
            )
            .await
            .expect("the sweep answers");
        common::flush().await;
        let answer = json!({ "deleted": deleted.iter().map(support::reaped_json).collect::<Vec<_>>(), "failed": failed.iter().map(support::failure_json).collect::<Vec<_>>() });
        support::check_create(&w, scene, answer);
    })
}

#[test]
fn strays_nothing_asks_with_no_backend_that_lists_by_name() {
    common::run_local(async {
        let fx = common::fixture("machines_creates");
        let scene = common::scene(
            &fx,
            "strays: nothing asks with no backend that lists by name",
        );
        let w = World::new(Opts {
            store: support::store_of(scene),
            ..Default::default()
        });
        let plain = w.provider(BackendOpts::default());
        let mut failed = Vec::new();
        let deleted = w
            .area
            .sweep_strays(
                &|| Vec::new(),
                &mut failed,
                &[plain as Arc<dyn MachineBackend>],
            )
            .await
            .expect("the sweep answers");
        common::flush().await;
        support::check_create(
            &w,
            scene,
            json!(deleted.iter().map(support::reaped_json).collect::<Vec<_>>()),
        );
    })
}
