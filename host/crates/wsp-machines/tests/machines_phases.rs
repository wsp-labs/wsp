// SPDX-License-Identifier: AGPL-3.0-only
//! The cases of machines_phases.md that no oracle scene replays: the late read, the gone confirmation's wait on the
//! clock, and the idle policy's wiring. Each expected value is read off machines.ts (the line is named) at
//! 65f6b89c7. The scenes are in machines_phases_scenes.rs. Spec: spec/machines_phases.md.

mod common;
#[path = "support/machines.rs"]
mod support;

use std::sync::Arc;

use serde_json::{json, Value};
use support::{from, BackendOpts, EntryOpts, Opts, World};
use wsp_engine::errors::EngineError;
use wsp_engine::machine::{MachineBackend, MachineState};
use wsp_machines::machines::machines::phases::GoneRead;
use wsp_machines::machines::machines::Refusal;
use wsp_wire::types::js_number::JsNumber;
use wsp_wire::views::session::SessionStatus;
use wsp_wire::wire::events_union::EventUnion;

fn with_lifecycle(backstop: bool, quiet: bool) -> Opts {
    Opts {
        backend: Some(BackendOpts {
            life: Some((1, backstop, quiet)),
            ..Default::default()
        }),
        ..Default::default()
    }
}

fn set_quiet(w: &World, quiet: Result<Option<u64>, EngineError>) {
    w.backend
        .life
        .as_ref()
        .expect("a lifecycle")
        .st
        .borrow_mut()
        .quiet = Some(quiet);
}

fn touched(id: &str) -> (&'static str, String) {
    ("touch", id.to_string())
}

fn forgot(id: &str) -> (&'static str, String) {
    ("forget", id.to_string())
}

/// machines.ts:271-279: one read `lateReadMs` after a wake gave up, which adopts a machine found running as any verb
/// would; arming again cancels the read armed before; a machine that does not run is left alone.
#[test]
fn a_late_read_after_a_given_up_wake_adopts_a_running_machine() {
    common::run_local(async {
        let w = World::new(Opts::default());
        let e = w.entry_in(EntryOpts {
            phase: "napping",
            state: Some(Ok(MachineState::Running)),
            ..Default::default()
        });

        w.area.arm_late_read(&e.entry);
        assert!(e.entry.borrow().late_read.is_some());
        assert_eq!(w.clock.pending(), 1);
        w.area.arm_late_read(&e.entry);
        assert_eq!(
            w.clock.pending(),
            1,
            "arming again cancels the read armed before"
        );

        w.clock.advance(180_000 - 1);
        common::flush().await;
        assert_eq!(
            e.phase(),
            wsp_wire::views::workspace::WorkspacePhase::Napping
        );
        assert!(
            support::log_of(&w.log_now(), &["machine.state"]).is_empty(),
            "nothing is read before lateReadMs"
        );

        w.clock.advance(1);
        common::flush().await;
        assert!(
            e.entry.borrow().late_read.is_none(),
            "the armed read is spent"
        );
        assert_eq!(
            e.phase(),
            wsp_wire::views::workspace::WorkspacePhase::Running,
            "adopted"
        );
        assert!(
            e.entry.borrow().unchecked,
            "reached like any running machine, unproven"
        );
        let log = w.log_now();
        assert_eq!(support::log_of(&log, &["machine.state"]).len(), 1);
        assert_eq!(
            support::log_of(&log, &["emit", "emitStatus"]),
            vec![
                json!(["emit", { "type": "workspace.woken", "workspaceId": "ws_1", "machineId": "m-1" }]),
                json!([
                    "emitStatus",
                    "ws_1",
                    "reachable",
                    "already running at the provider"
                ]),
            ]
        );

        // A machine that reads paused, or whose read fails (read as paused), is left as the record says.
        for state in [Ok(MachineState::Paused), Err(support::plain("timeout"))] {
            let quiet = World::new(Opts::default());
            let e = quiet.entry_in(EntryOpts {
                phase: "napping",
                state: Some(state),
                ..Default::default()
            });
            quiet.area.arm_late_read(&e.entry);
            quiet.clock.advance(180_000);
            common::flush().await;
            assert_eq!(
                e.phase(),
                wsp_wire::views::workspace::WorkspacePhase::Napping
            );
            let log = quiet.log_now();
            assert_eq!(
                support::names_of(&log),
                vec!["machine.state".to_string()],
                "one read and nothing else"
            );
            assert!(quiet.warns().is_empty());
        }
    })
}

/// machines.ts:322-325: the read waits `goneConfirmMs` on the clock when it is above 0, and reads at once when it is 0;
/// a read that fails is answered as the error.
#[test]
fn the_gone_confirmation_waits_gone_confirm_ms_on_the_clock() {
    common::run_local(async {
        let w = World::new(Opts {
            reads: Some(vec![Ok(MachineState::Gone)]),
            gone_confirm_ms: 5_000,
            ..Default::default()
        });
        let (area, backend) = (w.area.clone(), w.backend.clone() as Arc<dyn MachineBackend>);
        let read =
            support::started(async move { area.gone_confirmed(&backend, "m-1").await }).await;
        common::flush().await;
        assert!(!read.is_finished());
        assert_eq!(
            w.clock.pending(),
            1,
            "the wait is a timer on the injected clock"
        );
        assert!(w.log_now().is_empty(), "nothing is read yet");

        w.clock.advance(4_999);
        common::flush().await;
        assert!(!read.is_finished());
        assert!(w.log_now().is_empty());

        w.clock.advance(1);
        common::flush().await;
        assert_eq!(
            read.await.expect("ran"),
            GoneRead::State(MachineState::Gone)
        );
        assert_eq!(
            support::log_of(&w.log_now(), &["readGone"]),
            vec![json!(["readGone", "m-1"])]
        );

        // With no wait set, no timer is armed and the read is made at once; a failed read is the answer.
        let now = World::new(Opts {
            reads: Some(vec![Err(support::plain("503 down"))]),
            gone_confirm_ms: 0,
            ..Default::default()
        });
        let backend = now.backend.clone() as Arc<dyn MachineBackend>;
        let answer = now.area.gone_confirmed(&backend, "m-1").await;
        assert_eq!(answer, GoneRead::Failed("503 down".to_string()));
        assert_eq!(now.clock.pending(), 0);
    })
}

/// machines.ts:408-418: `None` for no entry or a record not running; the backend lifecycle's figure otherwise; a read
/// that fails is warned and read as `None`, and a backend that cannot say is not asked.
#[test]
fn idle_quiet_of_reads_the_figure_only_for_a_running_record() {
    common::run_local(async {
        let w = World::new(with_lifecycle(false, true));
        assert_eq!(w.area.quiet_of("ws_none").await, None, "no entry");

        let napping = w.entry_in(EntryOpts {
            phase: "napping",
            ..Default::default()
        });
        set_quiet(&w, Ok(Some(1234)));
        assert_eq!(w.area.quiet_of("ws_1").await, None, "a record not running");
        drop(napping);

        let running = World::new(with_lifecycle(false, true));
        let _e = running.entry_in(EntryOpts::default());
        set_quiet(&running, Ok(Some(1234)));
        assert_eq!(running.area.quiet_of("ws_1").await, Some(1234));
        set_quiet(&running, Ok(None));
        assert_eq!(running.area.quiet_of("ws_1").await, None);
        set_quiet(&running, Err(support::plain("the computer did not answer")));
        assert_eq!(running.area.quiet_of("ws_1").await, None);
        assert_eq!(
            running.warns(),
            vec!["quiet figure of ws_1 not read: the computer did not answer".to_string()]
        );

        let mute = World::new(with_lifecycle(false, false));
        let _e = mute.entry_in(EntryOpts::default());
        set_quiet(&mute, Ok(Some(1234)));
        assert_eq!(
            mute.area.quiet_of("ws_1").await,
            None,
            "a lifecycle that keeps no figure is not asked"
        );
    })
}

/// machines.ts:421-427, 433-437: a figure inside the window starts the window over; a figure at the window, none, or
/// one that fails goes on to the nap, named by the window in minutes.
#[test]
fn idle_a_quiet_figure_inside_the_window_resets_it() {
    common::run_local(async {
        let w = World::new(with_lifecycle(false, true));
        let e = w.entry_in(EntryOpts::default());

        set_quiet(&w, Ok(Some(60_000)));
        w.area
            .on_idle("ws_1", 300_000.0)
            .await
            .expect("the window starts over");
        assert_eq!(w.idle_calls(), vec![touched("ws_1")]);
        assert_eq!(
            e.phase(),
            wsp_wire::views::workspace::WorkspacePhase::Running
        );
        assert!(
            e.machine.s.borrow_mut().pauses.is_empty(),
            "nothing is stopped under somebody using it"
        );
        assert!(support::log_of(&w.log_now(), &["persist"]).is_empty());

        set_quiet(&w, Ok(Some(300_000)));
        w.area
            .on_idle("ws_1", 300_000.0)
            .await
            .expect("the nap lands");
        assert_eq!(
            e.phase(),
            wsp_wire::views::workspace::WorkspacePhase::Napping
        );
        assert_eq!(
            w.idle_calls(),
            vec![touched("ws_1"), forgot("ws_1")],
            "the napped event dropped the window"
        );
        let statuses = support::log_of(&w.log_now(), &["emitStatus"]);
        assert_eq!(
            statuses.last().unwrap(),
            &json!(["emitStatus", "ws_1", "napping", "idle 5 min"])
        );

        // A computer that cannot say, and one whose reading fails, leave the stop to this host's own clock.
        for quiet in [Ok(None), Err(support::plain("the computer did not answer"))] {
            let w = World::new(with_lifecycle(false, true));
            let e = w.entry_in(EntryOpts::default());
            set_quiet(&w, quiet);
            w.area
                .on_idle("ws_1", 180_000.0)
                .await
                .expect("the nap lands");
            assert_eq!(
                e.phase(),
                wsp_wire::views::workspace::WorkspacePhase::Napping
            );
            let statuses = support::log_of(&w.log_now(), &["emitStatus"]);
            assert_eq!(
                statuses.last().unwrap(),
                &json!(["emitStatus", "ws_1", "napping", "idle 3 min"])
            );
        }
    })
}

/// machines.ts:421-427: no entry is a window of nothing; the record's window otherwise; a refusal that stands raises it
/// to the backstop; a window that is off stays off.
#[test]
fn idle_a_standing_refusal_raises_the_window_to_the_backstop() {
    common::run_local(async {
        let w = World::new(Opts::default());
        assert_eq!(w.area.idle_window_of_id("ws_none"), None, "no entry");
        let e = w.entry_in(EntryOpts::default());
        assert_eq!(w.area.idle_window_of_id("ws_1"), Some(Some(1_200_000.0)));

        w.rt.nap_refusals.borrow_mut().insert(
            "ws_1".into(),
            Refusal {
                machine_id: "m-1".into(),
                said: "no pause".into(),
            },
        );
        assert_eq!(
            w.area.idle_window_of_id("ws_1"),
            Some(Some(2_400_000.0)),
            "asked again only at the backstop"
        );

        e.entry.borrow().record.borrow_mut().idle_window_ms = Some(Some(JsNumber(300_000.0)));
        assert_eq!(
            w.area.idle_window_of_id("ws_1"),
            Some(Some(600_000.0)),
            "twice the record's own window"
        );

        e.entry.borrow().record.borrow_mut().idle_window_ms = Some(None);
        assert_eq!(
            w.area.idle_window_of_id("ws_1"),
            Some(None),
            "a window that is off stays off"
        );
    })
}

/// machines.ts:441: a pause the backend never answered is the policy's to retry, so `on_idle` answers it as an error
/// and touches nothing.
#[test]
fn idle_a_move_unanswered_is_rethrown_for_the_policy_to_retry() {
    common::run_local(async {
        let w = World::new(Opts::default());
        let words = "pause did not complete in 20 s; the machine reads running";
        let e = w.entry_in(EntryOpts {
            nap: Some(EngineError::MoveUnanswered {
                message: words.into(),
                cause: None,
            }),
            ..Default::default()
        });

        let err = w
            .area
            .on_idle("ws_1", 300_000.0)
            .await
            .expect_err("the policy retries after retryMs");
        assert_eq!(err.message, words);
        assert!(w.idle_calls().is_empty(), "no touch: the deadline stands");
        assert_eq!(
            e.phase(),
            wsp_wire::views::workspace::WorkspacePhase::Running,
            "the nap put the phase back"
        );
        assert!(
            w.warns().iter().all(|l| !l.starts_with("idle nap of")),
            "{:?}",
            w.warns()
        );
    })
}

/// machines.ts:443: a machine the pause found gone settled its record on the way out, and the gone event dropped the
/// window: nothing is touched and nothing is said.
#[test]
fn idle_a_missing_nap_ends_quietly() {
    common::run_local(async {
        let w = World::new(Opts {
            reads: Some(vec![Ok(MachineState::Gone)]),
            ..Default::default()
        });
        let e = w.entry_in(EntryOpts {
            nap: Some(support::missing("404 not found")),
            ..Default::default()
        });

        w.area
            .on_idle("ws_1", 300_000.0)
            .await
            .expect("ends quietly");

        assert_eq!(e.phase(), wsp_wire::views::workspace::WorkspacePhase::Gone);
        assert_eq!(
            w.idle_calls(),
            vec![forgot("ws_1")],
            "the gone event dropped the window and nothing touched it"
        );
        assert!(
            w.warns().iter().all(|l| !l.starts_with("idle nap of")),
            "{:?}",
            w.warns()
        );
    })
}

/// machines.ts:444-451: any other failure starts a full window from the answer, pushes the row with the refusal's words
/// or the message, and warns unless it was a nap refusal (which was logged where the pause met it).
#[test]
fn idle_any_other_failure_touches_the_window_and_pushes_the_row() {
    common::run_local(async {
        let w = World::new(Opts::default());
        let _e = w.entry_in(EntryOpts {
            nap: Some(support::plain("upstream request timeout")),
            ..Default::default()
        });

        w.area
            .on_idle("ws_1", 300_000.0)
            .await
            .expect("a refused nap is the policy's to restart");

        assert_eq!(w.idle_calls(), vec![touched("ws_1")]);
        let statuses = support::log_of(&w.log_now(), &["emitStatus"]);
        let said = json!([
            "emitStatus",
            "ws_1",
            "reachable",
            "upstream request timeout"
        ]);
        assert_eq!(
            statuses[statuses.len() - 2..],
            [said.clone(), said],
            "the pause's own push, then the policy's with the same words"
        );
        assert_eq!(w.warns(), vec!["idle nap of ws_1 was answered with upstream request timeout; a full 5 min window starts over".to_string()]);

        // A nap refusal that stands pushes its own words and is not warned of again here.
        let refused = World::new(Opts::default());
        refused.rt.cfg.borrow_mut().nap_reason =
            Some("the provider will not pause this machine".into());
        let _e = refused.entry_in(EntryOpts {
            nap: Some(EngineError::nap_refused("m-1", "provider will not pause")),
            ..Default::default()
        });
        refused
            .area
            .on_idle("ws_1", 300_000.0)
            .await
            .expect("restarts the window");
        assert_eq!(refused.idle_calls(), vec![touched("ws_1")]);
        let statuses = support::log_of(&refused.log_now(), &["emitStatus"]);
        assert_eq!(
            statuses.last().unwrap(),
            &json!([
                "emitStatus",
                "ws_1",
                "reachable",
                "the provider will not pause this machine"
            ])
        );
        assert!(
            refused
                .warns()
                .iter()
                .all(|l| !l.starts_with("idle nap of")),
            "{:?}",
            refused.warns()
        );

        // A stop refusal reads as its own words where no nap refusal stands.
        let stopped = World::new(Opts::default());
        stopped.rt.cfg.borrow_mut().stop_reason =
            Some("the provider will not stop this machine".into());
        let _e = stopped.entry_in(EntryOpts {
            nap: Some(EngineError::stop_refused("m-1", "stop refused")),
            ..Default::default()
        });
        stopped
            .area
            .on_idle("ws_1", 300_000.0)
            .await
            .expect("restarts the window");
        let statuses = support::log_of(&stopped.log_now(), &["emitStatus"]);
        assert_eq!(
            statuses.last().unwrap(),
            &json!([
                "emitStatus",
                "ws_1",
                "reachable",
                "the provider will not stop this machine"
            ])
        );
    })
}

/// machines.ts:454: any running row on the workspace holds it awake; a finished row, or another workspace's, does not.
#[test]
fn idle_busy_holds_while_a_session_runs() {
    common::run_local(async {
        let w = World::new(Opts::default());
        let _e = w.entry_in(EntryOpts::default());
        assert!(!w.area.idle_busy("ws_1"), "no rows");

        w.running_row("s9", "ws_9");
        assert!(!w.area.idle_busy("ws_1"), "another workspace's row");

        w.running_row("s1", "ws_1");
        assert!(w.area.idle_busy("ws_1"), "a running row");

        w.rt.sessions
            .borrow()
            .get("s1")
            .unwrap()
            .borrow_mut()
            .view
            .status = SessionStatus::Completed;
        assert!(!w.area.idle_busy("ws_1"), "the window starts when it ends");
    })
}

/// machines.ts:460-467: the backstop reaches the lifecycle only for a live running record without a delete due; a
/// failing call is warned; a lifecycle with no backstop is not asked.
#[test]
fn idle_backstop_is_pushed_only_for_a_running_record() {
    common::run_local(async {
        let w = World::new(with_lifecycle(true, false));
        let e = w.entry_in(EntryOpts::default());
        let heard = |w: &World| {
            w.backend
                .life
                .as_ref()
                .unwrap()
                .st
                .borrow_mut()
                .backstops
                .clone()
        };

        w.area.on_backstop("ws_1", 1_790_002_400_000);
        common::flush().await;
        assert_eq!(
            heard(&w),
            vec![("m-1".to_string(), 1_790_002_400_000)],
            "the machine and the instant"
        );

        w.area.on_backstop("ws_none", 1);
        e.entry.borrow().record.borrow_mut().phase =
            wsp_wire::views::workspace::WorkspacePhase::Napping;
        w.area.on_backstop("ws_1", 2);
        e.entry.borrow().record.borrow_mut().phase =
            wsp_wire::views::workspace::WorkspacePhase::Running;
        e.entry.borrow().record.borrow_mut().sub = Some(from(json!({
            "on": "solari", "repo": "", "base": { "branch": "", "sha": "" }, "branch": "", "folder": "", "deleteAt": 5
        })));
        w.area.on_backstop("ws_1", 3);
        common::flush().await;
        assert_eq!(
            heard(&w).len(),
            1,
            "no entry, a napping record and a done sub-machine hear nothing"
        );

        e.entry.borrow().record.borrow_mut().sub = None;
        w.backend
            .life
            .as_ref()
            .unwrap()
            .st
            .borrow_mut()
            .backstop_err = Some(support::plain("PATCH ttlSeconds refused"));
        w.area.on_backstop("ws_1", 4);
        common::flush().await;
        assert_eq!(
            w.warns(),
            vec!["backstop of ws_1 on m-1 not set: PATCH ttlSeconds refused".to_string()]
        );

        let none = World::new(with_lifecycle(false, false));
        let _e = none.entry_in(EntryOpts::default());
        none.area.on_backstop("ws_1", 5);
        common::flush().await;
        assert!(
            none.backend
                .life
                .as_ref()
                .unwrap()
                .st
                .borrow_mut()
                .backstops
                .is_empty(),
            "a lifecycle with no backstop is never asked"
        );
    })
}

/// machines.ts:469-477: every road into a workspace starts its window over, and the three ends of a workspace drop it.
#[test]
fn idle_touches_and_forgets_on_the_bus_events() {
    common::run_local(async {
        let w = World::new(Opts::default());
        let on = |ty: &str, extra: Value, id: &str| -> EventUnion {
            let mut v = json!({ "type": ty, "workspaceId": id });
            for (k, x) in extra.as_object().unwrap() {
                v[k] = x.clone();
            }
            from(v)
        };
        let view = |id: &str| json!({ "id": id, "name": "w", "machineId": "m-1", "phase": "running", "golden": "", "createdAt": "", "project": { "id": "", "name": "", "path": "", "computer": "" } });
        let touching: Vec<EventUnion> = vec![
            on("session.start", json!({ "sessionId": "s" }), "ws_a"),
            on(
                "session.delta",
                json!({ "sessionId": "s", "kind": "text", "text": "x" }),
                "ws_b",
            ),
            on(
                "session.done",
                json!({ "sessionId": "s", "result": { "status": "completed" } }),
                "ws_c",
            ),
            on(
                "session.end",
                json!({ "sessionId": "s", "sawResult": true, "exitCode": null }),
                "ws_d",
            ),
            on(
                "session.steer",
                json!({ "sessionId": "s", "prompt": "p" }),
                "ws_e",
            ),
            on("inbox.file", json!({ "path": "/f", "bytes": 1 }), "ws_f"),
            on("workspace.woken", json!({ "machineId": "m-1" }), "ws_g"),
            on("workspace.upgraded", json!({ "machineId": "m-1" }), "ws_h"),
            on(
                "project.import",
                json!({ "source": "/s", "dest": "/d", "stage": "planned", "message": "m", "elapsedMs": 1 }),
                "ws_i",
            ),
            on(
                "project.export",
                json!({ "source": "/s", "dest": "/d", "stage": "packing", "message": "m", "elapsedMs": 1 }),
                "ws_j",
            ),
            from(json!({ "type": "workspace.created", "workspace": view("ws_k") })),
        ];
        for event in &touching {
            w.area.on_event(event);
        }
        let want: Vec<(&'static str, String)> = "abcdefghijk"
            .chars()
            .map(|c| touched(&format!("ws_{c}")))
            .collect();
        assert_eq!(
            w.idle_calls(),
            want,
            "each touches the workspace it names, `workspace.created` the one it carries"
        );

        let ending: Vec<EventUnion> = vec![
            on("workspace.napped", json!({}), "ws_l"),
            on(
                "workspace.gone",
                json!({ "machineId": "m-1", "reason": "r" }),
                "ws_m",
            ),
            on("workspace.deleted", json!({}), "ws_n"),
        ];
        for event in &ending {
            w.area.on_event(event);
        }
        let all = w.idle_calls();
        assert_eq!(
            all[want.len()..],
            [forgot("ws_l"), forgot("ws_m"), forgot("ws_n")]
        );

        // Anything else leaves the windows alone: a status that is not a paused running row touches nothing.
        w.area
            .on_event(&w.status_event("ws_o", "napping", "paused"));
        common::flush().await;
        assert_eq!(w.idle_calls().len(), all.len());
    })
}
