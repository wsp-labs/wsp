// SPDX-License-Identifier: AGPL-3.0-only
//! Ports test/places-setup.test.ts (the start, retry, sync and resume groups) and test/places-image.test.ts:204 (a
//! keyed frame waits out a gap) at the setup area's seam. Spec: spec/places_setup_area.md.
mod common;

use std::cell::Cell;
use std::rc::Rc;
use std::sync::Arc;

use serde_json::{json, Value};
use wsp_engine::errors::EngineError;
use wsp_engine::link_backend::LinkAsk;
use wsp_machines::places::setup::{GivenPicks, SetupPatch};
use wsp_machines::places::types::Resolved;
use wsp_wire::views::place::{PlaceSetup, PlaceSetupStep, PlaceSync};
use wsp_words::exit::Kind;
use wsp_words::views::place_setup_words::{
    no_setup_row_refusal, place_no_home_line, place_no_picks_line, place_provisioning_line,
    place_syncing_line,
};
use wsp_words::workspace_state::absent_computer;

use common::places::*;
use wsp_core::types::pending::Clock;

fn laptop() -> wsp_wire::types::recipe_file::RecipeFile {
    recipe(
        json!({ "name": "laptop", "agents": { "claude": { "signin": "vault" }, "codex": { "signin": "machine" } }, "clis": { "gh": { "via": "brew" } } }),
    )
}

fn setup(v: Value) -> PlaceSetup {
    serde_json::from_value(v).expect("a setup")
}

fn sync(v: Value) -> PlaceSync {
    serde_json::from_value(v).expect("a sync")
}

fn ask(key: Option<&str>, timeout_ms: Option<u64>) -> LinkAsk {
    LinkAsk {
        timeout_ms,
        idempotency_key: key.map(str::to_string),
    }
}

fn spoo(scene: &Scene, script: Script) -> Rc<FakeSocket> {
    scene.linked(
        "p_spoo",
        "spoo",
        report(
            "spoo",
            json!({ "daemonVersion": 200, "login": { "HOME": "/root", "PATH": "/usr/bin" } }),
        ),
        script,
    )
}

#[test]
fn setting_now_is_the_one_busy_reading() {
    let scene = Scene::new(SceneOpts::default());
    let mut r = record("p_spoo", "spoo", report("spoo", json!({})));
    assert_eq!(scene.setup.setting_now(&r), None);
    r.setup = Some(setup(
        json!({ "state": "running", "addId": "a1", "startedAt": iso(T0), "steps": [{ "step": "clis", "state": "running" }, { "step": "floor", "state": "done" }], "waiting": [] }),
    ));
    assert_eq!(
        scene.setup.setting_now(&r),
        Some(place_provisioning_line("spoo", Some(&PlaceSetupStep::Clis)))
    );
    r.setup = Some(setup(
        json!({ "state": "done", "addId": "a1", "startedAt": iso(T0), "steps": [], "waiting": [] }),
    ));
    assert_eq!(scene.setup.setting_now(&r), None);
    scene.setup.setting.borrow_mut().insert("p_spoo".into());
    assert_eq!(
        scene.setup.setting_now(&r),
        Some(place_provisioning_line("spoo", None))
    );
}

#[test]
fn woken_wakes_every_wait_and_a_late_one_leaves_the_next_set() {
    block(async {
        let scene = Scene::new(SceneOpts::default());
        scene
            .records
            .keep(record("p_spoo", "spoo", report("spoo", json!({}))));
        let at = scene.clock.now() as u64;
        let (a, b) = (Rc::clone(&scene.setup), Rc::clone(&scene.setup));
        let first =
            tokio::task::spawn_local(async move { a.dials_back("p_spoo", at + 10_000).await });
        let second =
            tokio::task::spawn_local(async move { b.dials_back("p_spoo", at + 10_000).await });
        settle().await;
        assert_eq!(
            scene.setup.waiting.borrow().get("p_spoo").map(|w| w.len()),
            Some(2)
        );
        scene.setup.woken("p_spoo", true);
        assert_eq!([first.await.unwrap(), second.await.unwrap()], [true, true]);
        assert!(scene.setup.waiting.borrow().get("p_spoo").is_none());
        let c = Rc::clone(&scene.setup);
        let third =
            tokio::task::spawn_local(async move { c.dials_back("p_spoo", at + 10_000).await });
        settle().await;
        scene.clock.pass(10_000).await;
        assert!(!third.await.unwrap(), "the wait ran out");
        assert_eq!(
            scene
                .setup
                .waiting
                .borrow()
                .get("p_spoo")
                .map(|w| w.len())
                .unwrap_or(0),
            0
        );
        assert_eq!(scene.clock.pending(), 0);
    });
}

#[test]
fn dials_back_answers_by_the_link_and_the_clock() {
    block(async {
        let scene = Scene::new(SceneOpts::default());
        let socket = spoo(&scene, answers_ok());
        let now = scene.clock.now() as u64;
        assert!(
            !scene.setup.dials_back("p_spoo", now + 5_000).await,
            "an open link is no gap"
        );
        socket.open.set(false);
        let s = Rc::clone(&scene.setup);
        let closing =
            tokio::task::spawn_local(async move { s.dials_back("p_spoo", now + 5_000).await });
        settle().await;
        assert!(!closing.is_finished(), "a closing socket is waited past");
        scene.setup.woken("p_spoo", true);
        assert!(closing.await.unwrap());
        assert!(
            !scene.setup.dials_back("p_spoo", now).await,
            "a wait already over answers false at once"
        );
        assert_eq!(scene.clock.pending(), 0);
    });
}

#[test]
fn a_frame_with_no_key_fails_at_once_on_an_absent_computer() {
    block(async {
        let scene = Scene::new(SceneOpts::default());
        let socket = spoo(&scene, answers_ok());
        scene.away("p_spoo", &socket);
        let link = scene.setup.link_to("p_spoo");
        let e = link
            .request("exec", Default::default(), ask(None, None))
            .await
            .expect_err("absent");
        assert!(matches!(e, EngineError::PlaceAbsent { .. }));
        assert_eq!(e.to_string(), absent_computer("spoo", None).sentence);
        assert_eq!(scene.clock.pending(), 0, "nothing waits");
    });
}

#[test]
fn a_keyed_frame_is_sent_again_on_the_link_that_comes_back() {
    block(async {
        let scene = Scene::new(SceneOpts {
            relink_wait_ms: Some(300),
            ..SceneOpts::default()
        });
        let socket = spoo(&scene, Rc::new(|_, _| Reply::Hang));
        let link = scene.setup.link_to("p_spoo");
        let l = Arc::clone(&link);
        let asked = tokio::task::spawn_local(async move {
            l.request("machine.create", Default::default(), ask(Some("k1"), None))
                .await
        });
        until("the frame went up", || {
            socket.reach.asked("machine.create") == 1
        })
        .await;
        scene.away("p_spoo", &socket);
        settle().await;
        scene.clock.pass(200).await;
        let again = scene.live(
            "p_spoo",
            Rc::new(|_, _| Reply::Ok(obj(json!({ "ok": true, "machine": { "id": "m1" } })))),
        );
        scene.setup.woken("p_spoo", true);
        let answer = asked.await.unwrap().expect("answered on the new link");
        assert_eq!(answer.get("machine"), Some(&json!({ "id": "m1" })));
        assert_eq!(again.reach.asked("machine.create"), 1);
        assert_eq!(socket.reach.asked("machine.create"), 1);
    });
}

#[test]
fn a_keyed_frame_fails_with_its_own_words_once_the_wait_runs_out() {
    block(async {
        let scene = Scene::new(SceneOpts {
            relink_wait_ms: Some(300),
            ..SceneOpts::default()
        });
        let socket = spoo(&scene, Rc::new(|_, _| Reply::Hang));
        let link = scene.setup.link_to("p_spoo");
        let l = Arc::clone(&link);
        let asked = tokio::task::spawn_local(async move {
            l.request("machine.create", Default::default(), ask(Some("k1"), None))
                .await
        });
        until("the frame went up", || {
            socket.reach.asked("machine.create") == 1
        })
        .await;
        let closed = scene.clock.now();
        scene.away("p_spoo", &socket);
        scene.clock.pass(299).await;
        assert!(!asked.is_finished(), "still inside the wait");
        scene.clock.pass(1).await;
        let e = asked.await.unwrap().expect_err("the wait ran out");
        assert_eq!(e.to_string(), "connection lost");
        assert!(scene.clock.now() - closed >= 300);
    });
}

#[test]
fn a_keyed_frame_to_a_computer_never_here_is_refused_at_once() {
    block(async {
        let scene = Scene::new(SceneOpts::default());
        scene
            .records
            .keep(record("p_spoo", "spoo", report("spoo", json!({}))));
        let e = scene
            .setup
            .link_to("p_spoo")
            .request("exec", Default::default(), ask(Some("k1"), None))
            .await
            .expect_err("absent");
        assert!(matches!(e, EngineError::PlaceAbsent { .. }));
        assert_eq!(scene.clock.pending(), 0);
    });
}

#[test]
fn the_places_own_refusal_is_never_waited_out() {
    block(async {
        let scene = Scene::new(SceneOpts::default());
        let socket = spoo(
            &scene,
            Rc::new(|_, _| Reply::Err("no such machine: k1".into())),
        );
        for key in [None, Some("k1")] {
            let e = scene
                .setup
                .link_to("p_spoo")
                .request("machine.get", Default::default(), ask(key, None))
                .await
                .expect_err("refused");
            assert_eq!(e.to_string(), "no such machine: k1");
        }
        assert_eq!(socket.reach.asked("machine.get"), 2);
        assert_eq!(scene.clock.pending(), 0);
    });
}

#[test]
fn a_frame_is_bounded_by_its_own_timeout_or_the_frame_wait() {
    block(async {
        let scene = Scene::new(SceneOpts {
            frame_wait_ms: Some(4_000),
            ..SceneOpts::default()
        });
        let _socket = spoo(&scene, Rc::new(|_, _| Reply::Hang));
        let link = scene.setup.link_to("p_spoo");
        let (a, b) = (Arc::clone(&link), Arc::clone(&link));
        let own = tokio::task::spawn_local(async move {
            a.request("machine.state", Default::default(), ask(None, Some(1_500)))
                .await
        });
        let default = tokio::task::spawn_local(async move {
            b.request("exec", Default::default(), ask(None, None)).await
        });
        settle().await;
        scene.clock.pass(1_500).await;
        assert_eq!(
            own.await.unwrap().err().unwrap().to_string(),
            "machine.state on spoo was not answered in 2s"
        );
        assert!(!default.is_finished());
        scene.clock.pass(2_500).await;
        assert_eq!(
            default.await.unwrap().err().unwrap().to_string(),
            "exec on spoo was not answered in 4s"
        );
    });
}

#[test]
fn write_setup_keeps_a_field_another_writer_set_meanwhile() {
    block(async {
        let scene = Scene::new(SceneOpts::default());
        let mut r = record("p_spoo", "spoo", report("spoo", json!({})));
        r.sync = Some(sync(
            json!({ "state": "behind", "changes": ["clis/jq"], "since": iso(T0) }),
        ));
        scene.records.keep(r);
        *scene.records.meanwhile.borrow_mut() = Some(Box::new(|r| {
            r.last_seen_at = "2026-10-25T01:00:00.000Z".into()
        }));
        let started = setup(
            json!({ "state": "running", "addId": "a1", "startedAt": iso(T0), "steps": [], "waiting": [] }),
        );
        scene
            .setup
            .write_setup(
                "p_spoo",
                SetupPatch {
                    setup: Some(started.clone()),
                    ..Default::default()
                },
            )
            .await
            .unwrap();
        let now = scene.records.get("p_spoo");
        assert_eq!(now.last_seen_at, "2026-10-25T01:00:00.000Z");
        assert_eq!(now.setup, Some(started));
        assert!(now.sync.is_some(), "a patch that leaves sync out keeps it");
        scene
            .setup
            .write_setup(
                "p_spoo",
                SetupPatch {
                    sync: Some(None),
                    ..Default::default()
                },
            )
            .await
            .unwrap();
        assert_eq!(scene.records.get("p_spoo").sync, None);
        assert_eq!(scene.ctx.kept.borrow().get("p_spoo").unwrap().sync, None);
    });
}

#[test]
fn start_refuses_or_says_in_the_ts_order() {
    block(async {
        let none = Scene::new(SceneOpts {
            provision: false,
            ..SceneOpts::default()
        });
        spoo(&none, answers_ok());
        let said = none
            .setup
            .start_setup(
                "p_spoo",
                "a1",
                Some(GivenPicks {
                    picks: laptop(),
                    recipe: None,
                }),
                None,
            )
            .await
            .unwrap();
        assert_eq!(
            (said.setup, said.said),
            (None, None),
            "a host that wired no provisioner sets nothing up"
        );

        let scene = Scene::new(SceneOpts::default());
        let said = scene
            .setup
            .start_setup("p_gone", "a1", None, None)
            .await
            .unwrap();
        assert_eq!((said.setup, said.said), (None, None));

        scene.records.keep(record(
            "p_bare",
            "bare",
            report("bare", json!({ "login": { "PATH": "/usr/bin" } })),
        ));
        let said = scene
            .setup
            .start_setup(
                "p_bare",
                "a1",
                Some(GivenPicks {
                    picks: laptop(),
                    recipe: None,
                }),
                None,
            )
            .await
            .unwrap();
        assert_eq!(said.said, Some(place_no_home_line("bare")));

        spoo(&scene, answers_ok());
        let said = scene
            .setup
            .start_setup("p_spoo", "a1", None, None)
            .await
            .unwrap();
        assert_eq!(said.said, Some(place_no_picks_line("spoo")));

        scene.setup.syncing.borrow_mut().insert("p_spoo".into());
        let e = scene
            .setup
            .start_setup(
                "p_spoo",
                "a1",
                Some(GivenPicks {
                    picks: laptop(),
                    recipe: None,
                }),
                None,
            )
            .await
            .expect_err("syncing");
        assert_eq!(
            (e.message.as_str(), e.kind),
            (place_syncing_line("spoo").as_str(), Some(Kind::Conflict))
        );
        scene.setup.setting.borrow_mut().insert("p_spoo".into());
        let e = scene
            .setup
            .start_setup(
                "p_spoo",
                "a1",
                Some(GivenPicks {
                    picks: laptop(),
                    recipe: None,
                }),
                None,
            )
            .await
            .expect_err("busy");
        assert_eq!(
            (e.message.as_str(), e.kind),
            (
                place_provisioning_line("spoo", None).as_str(),
                Some(Kind::Conflict)
            )
        );
        assert!(scene.provisioner.picked.borrow().is_empty());
    });
}

#[test]
fn start_writes_the_running_setup_before_the_run() {
    block(async {
        let scene = Scene::new(SceneOpts::default());
        let socket = spoo(&scene, answers_ok());
        scene
            .provisioner
            .hold(wsp_engine::provision::EngineStep::Floor);
        let started = scene
            .setup
            .start_setup(
                "p_spoo",
                "a_mine",
                Some(GivenPicks {
                    picks: laptop(),
                    recipe: Some("laptop".into()),
                }),
                None,
            )
            .await
            .unwrap();
        let expect = setup(
            json!({ "state": "running", "addId": "a_mine", "startedAt": iso(T0), "steps": [], "waiting": [] }),
        );
        assert_eq!(started.setup, Some(expect.clone()));
        let r = scene.records.get("p_spoo");
        assert_eq!(r.setup, Some(expect));
        assert_eq!(r.picks, Some(laptop()));
        assert_eq!(r.recipe.as_deref(), Some("laptop"));
        assert!(scene.setup.setting.borrow().contains("p_spoo"));
        let none = Scene::new(SceneOpts::default());
        spoo(&none, answers_ok());
        none.provisioner
            .hold(wsp_engine::provision::EngineStep::Floor);
        none.setup
            .start_setup(
                "p_spoo",
                "a2",
                Some(GivenPicks {
                    picks: laptop(),
                    recipe: None,
                }),
                None,
            )
            .await
            .unwrap();
        assert_eq!(
            none.records.get("p_spoo").recipe.as_deref(),
            Some("none"),
            "picks given with no recipe follow none"
        );
        drop(socket);
    });
}

#[test]
fn a_resume_keeps_the_ended_steps_and_asks_again_for_a_waiting_sign_in() {
    block(async {
        let scene = Scene::new(SceneOpts::default());
        spoo(&scene, answers_ok());
        let mut r = scene.records.get("p_spoo");
        r.picks = Some(laptop());
        r.setup = Some(setup(json!({
            "state": "running", "addId": "a_old", "startedAt": "2026-10-24T10:00:00.000Z",
            "steps": [{ "step": "floor", "state": "done", "ms": 5 }, { "step": "agents", "state": "done", "ms": 9 }, { "step": "signins", "state": "done", "ms": 1 }, { "step": "clis", "state": "running" }],
            "waiting": [{ "row": "signins/codex", "label": "Codex", "expiresAt": "2026-10-24T10:15:00.000Z", "state": "waiting", "code": "OLD" }],
        })));
        scene.records.keep(r);
        scene
            .provisioner
            .hold(wsp_engine::provision::EngineStep::Clis);
        let started = scene
            .setup
            .start_setup("p_spoo", "a_old", None, None)
            .await
            .unwrap();
        let s = started.setup.expect("resumed");
        assert_eq!(s.started_at, "2026-10-24T10:00:00.000Z");
        assert_eq!(
            s.steps
                .iter()
                .map(|l| serde_json::to_value(l).unwrap())
                .collect::<Vec<_>>(),
            vec![
                json!({ "step": "floor", "state": "done", "ms": 5 }),
                json!({ "step": "agents", "state": "done", "ms": 9 }),
                json!({ "step": "signins", "state": "done", "ms": 1 }),
            ]
        );
        assert!(s.waiting.is_empty());
        until("the run reached the clis", || {
            scene.provisioner.ran_words().contains(&"clis")
        })
        .await;
        assert!(!scene.provisioner.ran_words().contains(&"floor"));
        assert!(!scene.provisioner.ran_words().contains(&"agents"));
    });
}

#[test]
fn a_row_retry_runs_its_step_its_followers_and_the_context() {
    block(async {
        let scene = Scene::new(SceneOpts::default());
        spoo(&scene, answers_ok());
        let mut r = scene.records.get("p_spoo");
        r.picks = Some(laptop());
        let lines: Vec<Value> = [
            "floor",
            "agents",
            "signins",
            "skills",
            "github",
            "clis",
            "mcp",
            "configs",
            "plugins",
            "folders",
            "folderServers",
            "context",
        ]
        .iter()
        .map(|s| json!({ "step": s, "state": "done", "ms": 1 }))
        .collect();
        r.setup = Some(setup(
            json!({ "state": "done", "addId": "a1", "startedAt": iso(T0), "finishedAt": iso(T0), "steps": lines, "waiting": [] }),
        ));
        r.applied = Some(serde_json::from_value(json!({ "hash": "h", "at": iso(T0), "rows": [{ "id": "tools/brew/gh", "label": "GitHub CLI", "outcome": "failed", "step": "clis" }] })).unwrap());
        scene.records.keep(r);
        let started = scene
            .setup
            .start_setup("p_spoo", "a2", None, Some("tools/brew/gh"))
            .await
            .unwrap();
        let kept: Vec<String> = started
            .setup
            .unwrap()
            .steps
            .iter()
            .map(|l| {
                serde_json::to_value(l.step)
                    .unwrap()
                    .as_str()
                    .unwrap()
                    .to_string()
            })
            .collect();
        assert_eq!(
            kept,
            ["floor", "agents", "signins", "skills", "github", "configs", "plugins", "folders"]
        );
        until("the run ended", || {
            !scene.setup.setting.borrow().contains("p_spoo")
        })
        .await;
        assert_eq!(scene.provisioner.ran_words(), ["clis", "mcp", "context"]);
    });
}

#[test]
fn a_row_the_setup_holds_nothing_under_is_refused() {
    block(async {
        let scene = Scene::new(SceneOpts::default());
        spoo(&scene, answers_ok());
        let mut r = scene.records.get("p_spoo");
        r.picks = Some(laptop());
        r.setup = Some(setup(
            json!({ "state": "done", "addId": "a1", "startedAt": iso(T0), "steps": [], "waiting": [] }),
        ));
        scene.records.keep(r);
        let e = scene
            .setup
            .start_setup("p_spoo", "a2", None, Some("nothing/here"))
            .await
            .expect_err("refused");
        assert_eq!(
            e.message,
            wsp_words::exit::usage_refusal(
                &no_setup_row_refusal("nothing/here", "spoo"),
                "Run again a row the computer's page shows."
            )
            .message
        );
        assert_eq!(e.kind, Some(Kind::Usage));
        assert!(
            !scene.setup.setting.borrow().contains("p_spoo"),
            "a refusal lets the computer go"
        );
    });
}

#[test]
fn the_end_of_a_run_lets_the_computer_go_and_syncs_a_change_that_waited() {
    block(async {
        let shelf = Rc::new(Shelf {
            now: std::cell::RefCell::new(Resolved {
                file: laptop(),
                items: Default::default(),
                hash: "h2".into(),
            }),
            resolves: Cell::new(0),
        });
        let scene = Scene::new(SceneOpts {
            shelf: Some(Rc::clone(&shelf)),
            ..SceneOpts::default()
        });
        spoo(&scene, answers_ok());
        scene
            .provisioner
            .hold(wsp_engine::provision::EngineStep::Floor);
        scene
            .setup
            .start_setup(
                "p_spoo",
                "a1",
                Some(GivenPicks {
                    picks: laptop(),
                    recipe: Some("laptop".into()),
                }),
                None,
            )
            .await
            .unwrap();
        scene.setup.sync_place("p_spoo").await.unwrap();
        assert!(
            scene.setup.again.borrow().contains("p_spoo"),
            "a change during a setup waits for its end"
        );
        assert_eq!(shelf.resolves.get(), 1, "only the run's own resolve so far");
        scene
            .provisioner
            .let_go(wsp_engine::provision::EngineStep::Floor);
        until("the run ended", || {
            !scene.setup.setting.borrow().contains("p_spoo")
        })
        .await;
        scene.clock.pass(0).await;
        assert!(!scene.setup.again.borrow().contains("p_spoo"));
        assert_eq!(
            shelf.resolves.get(),
            2,
            "the waiting change synced once at the end"
        );
    });
}

#[test]
fn started_or_said_says_a_refusal() {
    block(async {
        let scene = Scene::new(SceneOpts::default());
        spoo(&scene, answers_ok());
        scene.setup.setting.borrow_mut().insert("p_spoo".into());
        let said = scene
            .setup
            .started_or_said(
                "p_spoo",
                "a1",
                Some(GivenPicks {
                    picks: laptop(),
                    recipe: None,
                }),
            )
            .await;
        assert_eq!(
            (said.setup, said.said),
            (None, Some(place_provisioning_line("spoo", None)))
        );
    });
}

#[test]
fn sync_of_reads_nothing_where_nothing_is_followed() {
    block(async {
        let shelf = Rc::new(Shelf {
            now: std::cell::RefCell::new(Resolved {
                file: laptop(),
                items: Default::default(),
                hash: "h1".into(),
            }),
            resolves: Cell::new(0),
        });
        let scene = Scene::new(SceneOpts {
            shelf: Some(Rc::clone(&shelf)),
            ..SceneOpts::default()
        });
        let mut r = record("p_spoo", "spoo", report("spoo", json!({})));
        assert!(scene.setup.sync_of(&r, None).await.is_none(), "no recipe");
        r.recipe = Some("none".into());
        assert!(
            scene.setup.sync_of(&r, None).await.is_none(),
            "follows none"
        );
        r.recipe = Some("laptop".into());
        assert!(scene.setup.sync_of(&r, None).await.is_none(), "no picks");
        r.picks = Some(laptop());
        assert!(scene.setup.sync_of(&r, None).await.is_none(), "no setup");
        r.setup = Some(setup(
            json!({ "state": "running", "addId": "a1", "startedAt": iso(T0), "steps": [], "waiting": [] }),
        ));
        assert!(
            scene.setup.sync_of(&r, None).await.is_none(),
            "a setup still running"
        );
        r.setup = Some(setup(
            json!({ "state": "done", "addId": "a1", "startedAt": iso(T0), "steps": [], "waiting": [] }),
        ));
        r.applied = Some(
            serde_json::from_value(json!({ "hash": "h1", "at": iso(T0), "rows": [] })).unwrap(),
        );
        let read = scene
            .setup
            .sync_of(&r, None)
            .await
            .expect("followed and in step");
        assert!(read.changes.is_empty());
        assert_eq!(read.resolved.hash, "h1");
        assert_eq!(
            shelf.resolves.get(),
            1,
            "the shelf is read only once every other reason passed"
        );
    });
}

#[test]
fn a_recipe_that_moved_no_row_runs_nothing() {
    block(async {
        let renamed = recipe(
            json!({ "name": "laptop two", "agents": { "claude": { "signin": "vault" }, "codex": { "signin": "machine" } }, "clis": { "gh": { "via": "brew" } } }),
        );
        let shelf = Rc::new(Shelf {
            now: std::cell::RefCell::new(Resolved {
                file: renamed.clone(),
                items: Default::default(),
                hash: "h2".into(),
            }),
            resolves: Cell::new(0),
        });
        let scene = Scene::new(SceneOpts {
            shelf: Some(shelf),
            ..SceneOpts::default()
        });
        spoo(&scene, answers_ok());
        let mut r = scene.records.get("p_spoo");
        r.recipe = Some("laptop".into());
        r.picks = Some(laptop());
        r.setup = Some(setup(
            json!({ "state": "done", "addId": "a1", "startedAt": iso(T0), "steps": [], "waiting": [] }),
        ));
        r.applied = Some(
            serde_json::from_value(json!({ "hash": "h1", "at": iso(T0), "rows": [] })).unwrap(),
        );
        r.sync = Some(sync(
            json!({ "state": "behind", "changes": ["name"], "since": iso(T0) }),
        ));
        scene.records.keep(r);
        scene.setup.sync_place("p_spoo").await.unwrap();
        let now = scene.records.get("p_spoo");
        assert_eq!(now.applied.as_ref().map(|a| a.hash.as_str()), Some("h2"));
        assert_eq!(now.picks, Some(renamed));
        assert_eq!(now.sync, None);
        assert!(scene.provisioner.ran.borrow().is_empty());
        let frames = scene.sync_frames();
        assert_eq!(frames.len(), 1);
        assert_eq!(frames[0].sync, None);
    });
}

#[test]
fn a_computer_away_reads_behind() {
    block(async {
        let moved = recipe(
            json!({ "name": "laptop", "agents": { "claude": { "signin": "vault" }, "codex": { "signin": "machine" } }, "clis": { "gh": { "via": "brew" }, "jq": { "via": "brew" } } }),
        );
        let shelf = Rc::new(Shelf {
            now: std::cell::RefCell::new(Resolved {
                file: moved,
                items: Default::default(),
                hash: "h2".into(),
            }),
            resolves: Cell::new(0),
        });
        let scene = Scene::new(SceneOpts {
            shelf: Some(shelf),
            ..SceneOpts::default()
        });
        let socket = spoo(&scene, answers_ok());
        let mut r = scene.records.get("p_spoo");
        r.recipe = Some("laptop".into());
        r.picks = Some(laptop());
        r.setup = Some(setup(
            json!({ "state": "done", "addId": "a1", "startedAt": iso(T0), "steps": [], "waiting": [] }),
        ));
        r.applied = Some(
            serde_json::from_value(json!({ "hash": "h1", "at": iso(T0), "rows": [] })).unwrap(),
        );
        scene.records.keep(r);
        scene.away("p_spoo", &socket);
        scene.setup.sync_place("p_spoo").await.unwrap();
        let s = scene.records.get("p_spoo").sync.expect("behind");
        assert_eq!(
            serde_json::to_value(&s).unwrap(),
            json!({ "state": "behind", "changes": ["clis/jq"], "since": iso(T0) })
        );
        assert!(scene.provisioner.ran.borrow().is_empty());
        assert!(!scene.setup.syncing.borrow().contains("p_spoo"));
    });
}

#[test]
fn a_change_mid_sync_runs_once_more_at_its_end() {
    block(async {
        let moved = recipe(
            json!({ "name": "laptop", "agents": { "claude": { "signin": "vault" }, "codex": { "signin": "machine" } }, "clis": { "gh": { "via": "brew" } }, "skills": { "review": { "from": "github.com/acme/skills" } } }),
        );
        let shelf = Rc::new(Shelf {
            now: std::cell::RefCell::new(Resolved {
                file: moved,
                items: Default::default(),
                hash: "h2".into(),
            }),
            resolves: Cell::new(0),
        });
        let scene = Scene::new(SceneOpts {
            shelf: Some(Rc::clone(&shelf)),
            ..SceneOpts::default()
        });
        spoo(&scene, answers_ok());
        let mut r = scene.records.get("p_spoo");
        r.recipe = Some("laptop".into());
        r.picks = Some(laptop());
        r.setup = Some(setup(
            json!({ "state": "done", "addId": "a1", "startedAt": iso(T0), "steps": [], "waiting": [] }),
        ));
        r.applied = Some(
            serde_json::from_value(json!({ "hash": "h1", "at": iso(T0), "rows": [] })).unwrap(),
        );
        scene.records.keep(r);
        scene
            .provisioner
            .hold(wsp_engine::provision::EngineStep::Skills);
        let s = Rc::clone(&scene.setup);
        let first = tokio::task::spawn_local(async move { s.sync_place("p_spoo").await });
        until("the sync reached the skills", || {
            scene.provisioner.ran_words().contains(&"skills")
        })
        .await;
        scene.setup.sync_place("p_spoo").await.unwrap();
        assert!(scene.setup.again.borrow().contains("p_spoo"));
        scene
            .provisioner
            .let_go(wsp_engine::provision::EngineStep::Skills);
        first.await.unwrap().unwrap();
        scene.clock.pass(0).await;
        settle().await;
        assert_eq!(
            shelf.resolves.get(),
            2,
            "once more at its end, and only once"
        );
        assert!(!scene.setup.again.borrow().contains("p_spoo"));
    });
}

#[test]
fn sync_soon_folds_a_burst_into_one() {
    block(async {
        let shelf = Rc::new(Shelf {
            now: std::cell::RefCell::new(Resolved {
                file: laptop(),
                items: Default::default(),
                hash: "h1".into(),
            }),
            resolves: Cell::new(0),
        });
        let scene = Scene::new(SceneOpts {
            shelf: Some(Rc::clone(&shelf)),
            ..SceneOpts::default()
        });
        spoo(&scene, answers_ok());
        let mut r = scene.records.get("p_spoo");
        r.recipe = Some("laptop".into());
        r.picks = Some(laptop());
        r.setup = Some(setup(
            json!({ "state": "done", "addId": "a1", "startedAt": iso(T0), "steps": [], "waiting": [] }),
        ));
        r.applied = Some(
            serde_json::from_value(json!({ "hash": "h1", "at": iso(T0), "rows": [] })).unwrap(),
        );
        scene.records.keep(r);
        scene.setup.sync_soon("p_spoo", 1_000);
        scene.clock.pass(600).await;
        scene.setup.sync_soon("p_spoo", 1_000);
        scene.clock.pass(600).await;
        assert_eq!(
            shelf.resolves.get(),
            0,
            "the clock starts again at every ask"
        );
        scene.clock.pass(400).await;
        assert_eq!(shelf.resolves.get(), 1);
        assert!(scene.setup.sync_timers.borrow().is_empty());
    });
}

#[test]
fn mark_sync_writes_nothing_with_nothing_to_clear() {
    block(async {
        let scene = Scene::new(SceneOpts::default());
        scene
            .records
            .keep(record("p_spoo", "spoo", report("spoo", json!({}))));
        scene.setup.mark_sync("p_spoo", None).await.unwrap();
        scene.setup.mark_sync("p_gone", None).await.unwrap();
        assert!(scene.records.writes.borrow().is_empty());
        assert!(scene.sync_frames().is_empty());
        let behind = sync(json!({ "state": "behind", "changes": ["clis/jq"], "since": iso(T0) }));
        scene
            .setup
            .mark_sync("p_spoo", Some(behind.clone()))
            .await
            .unwrap();
        assert_eq!(scene.records.get("p_spoo").sync, Some(behind.clone()));
        assert_eq!(
            scene.sync_frames().last().and_then(|f| f.sync.clone()),
            Some(behind)
        );
    });
}

#[test]
fn the_log_goes_up_in_batches_every_line_stamped() {
    block(async {
        let scene = Scene::new(SceneOpts::default());
        let socket = spoo(&scene, answers_ok());
        scene
            .provisioner
            .hold(wsp_engine::provision::EngineStep::Agents);
        scene
            .setup
            .start_setup(
                "p_spoo",
                "a1",
                Some(GivenPicks {
                    picks: laptop(),
                    recipe: None,
                }),
                None,
            )
            .await
            .unwrap();
        until("the agents step started", || {
            scene.provisioner.ran_words().contains(&"agents")
        })
        .await;
        let appends = || {
            socket
                .reach
                .cmds()
                .into_iter()
                .filter(|c| c.contains("/.wsp/provision/log") && c.contains("printf"))
                .count()
        };
        assert_eq!(
            appends(),
            0,
            "nothing goes up before the batch's two seconds"
        );
        scene.clock.pass(2_000).await;
        assert_eq!(
            appends(),
            1,
            "one append carries the header and every line so far"
        );
        let stamped = wsp_words::views::place_setup_words::provision_log_line(
            T0 as f64,
            &format!("wsp zingzys-mac set up spoo from laptop at {}", iso(T0)),
        );
        let text: String = socket
            .reach
            .cmds()
            .iter()
            .filter(|c| c.contains("/.wsp/provision/log") && c.contains("printf"))
            .map(|c| common::places::written(c))
            .collect();
        assert!(
            text.starts_with(&stamped),
            "the header goes first, stamped when it was written: {text}"
        );
        scene
            .provisioner
            .let_go(wsp_engine::provision::EngineStep::Agents);
        until("the run ended", || {
            !scene.setup.setting.borrow().contains("p_spoo")
        })
        .await;
    });
}

#[test]
fn the_log_close_writes_the_result_beside_it() {
    block(async {
        let scene = Scene::new(SceneOpts::default());
        let socket = spoo(&scene, answers_ok());
        scene
            .setup
            .start_setup(
                "p_spoo",
                "a1",
                Some(GivenPicks {
                    picks: laptop(),
                    recipe: None,
                }),
                None,
            )
            .await
            .unwrap();
        until("the run ended", || {
            !scene.setup.setting.borrow().contains("p_spoo")
        })
        .await;
        settle().await;
        let cmds = socket.reach.cmds();
        assert!(
            cmds.iter()
                .any(|c| c.contains("/root/.wsp/provision/") && c.contains("result")),
            "the result file goes beside the log: {cmds:?}"
        );
    });
}

#[test]
fn a_log_the_computer_will_not_take_fails_nothing() {
    block(async {
        let scene = Scene::new(SceneOpts::default());
        let script: Script = Rc::new(|op, p| {
            let cmd = p.get("cmd").and_then(Value::as_str).unwrap_or("");
            if op == "exec" && cmd.contains("/.wsp/provision/") {
                Reply::Err("disk full".into())
            } else if op == "exec" {
                Reply::Ok(obj(
                    json!({ "ok": true, "exitCode": 0, "stdout": "", "stderr": "", "truncated": false }),
                ))
            } else {
                Reply::Ok(obj(json!({ "ok": true })))
            }
        });
        spoo(&scene, script);
        scene
            .setup
            .start_setup(
                "p_spoo",
                "a1",
                Some(GivenPicks {
                    picks: recipe(
                        json!({ "name": "laptop", "agents": { "claude": { "signin": "vault" } } }),
                    ),
                    recipe: None,
                }),
                None,
            )
            .await
            .unwrap();
        until("the run ended", || {
            !scene.setup.setting.borrow().contains("p_spoo")
        })
        .await;
        let r = scene.records.get("p_spoo");
        assert_eq!(
            serde_json::to_value(r.setup.unwrap().state).unwrap(),
            json!("done")
        );
    });
}
