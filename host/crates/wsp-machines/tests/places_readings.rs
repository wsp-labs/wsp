// SPDX-License-Identifier: AGPL-3.0-only
//! Ports test/box-panels.test.ts:693-860 (the readings of a joined computer) at the readings' seam, and pins the door
//! context of context.ts. Spec: spec/places_readings.md.
mod common;

use std::cell::RefCell;
use std::rc::Rc;

use serde_json::json;
use wsp_machines::places::readings::{PlaceReadings, Reader};
use wsp_machines::places::types::PlaceEvent;
use wsp_wire::wire::daemon_events::DaemonEvent;
use wsp_wire::wire::machine_link::SysSample;
use wsp_wire::wire::machine_link::no_such_place_refusal;

use common::places::*;

const CURRENT: i64 = 200;
const BEFORE_UNWATCH: i64 = 151;

fn sample(at: i64) -> SysSample {
    serde_json::from_value(json!({ "cpu": 12, "load1": 0.4, "mem": { "used": 1, "total": 4 }, "disk": { "used": 2, "total": 9 }, "at": at })).expect("a sample")
}

fn reader() -> (Reader, Rc<RefCell<Vec<SysSample>>>) {
    let got = Rc::new(RefCell::new(Vec::new()));
    let g = Rc::clone(&got);
    (Rc::new(move |s: &SysSample| g.borrow_mut().push(s.clone())), got)
}

fn scene_with(version: i64, script: Script) -> (Scene, Rc<FakeSocket>, Rc<PlaceReadings>) {
    let scene = Scene::new(SceneOpts::default());
    let socket = scene.linked("p_spoo", "spoo", report("spoo", json!({ "daemonVersion": version })), script);
    let readings = PlaceReadings::new(Rc::clone(&scene.ctx));
    (scene, socket, readings)
}

fn heard(readings: &PlaceReadings, at: i64) {
    readings.heard("p_spoo", &DaemonEvent::SysSample(sample(at)));
}

#[test]
fn the_context_takes_the_ts_defaults() {
    let scene = Scene::new(SceneOpts::default());
    assert_eq!([scene.ctx.seen_every_ms, scene.ctx.dial_wait_ms, scene.ctx.frame_wait_ms, scene.ctx.relink_wait_ms], [60_000, 20_000, 300_000, 120_000]);
    assert!(scene.ctx.live.borrow().is_empty());
    assert!(scene.ctx.kept.borrow().is_empty());
    assert!(scene.ctx.backends.borrow().is_empty());
    assert!(scene.ctx.forwards.borrow().is_empty());
    assert!(scene.ctx.asking.borrow().is_empty());
    let set = Scene::new(SceneOpts { relink_wait_ms: Some(300), frame_wait_ms: Some(50), ..SceneOpts::default() });
    assert_eq!([set.ctx.frame_wait_ms, set.ctx.relink_wait_ms], [50, 300]);
}

#[test]
fn sign_ins_here_reads_the_record_as_it_stands() {
    let scene = Scene::new(SceneOpts::default());
    assert_eq!(scene.ctx.sign_ins_here("p_none", None), None);
    scene.records.keep(record("p_spoo", "spoo", report("spoo", json!({ "agents": ["claude", "codex"], "logins": [] }))));
    let words = |s: &Scene| serde_json::to_value(s.ctx.sign_ins_here("p_spoo", None)).unwrap();
    assert_eq!(words(&scene), json!({ "claude": "vault-key", "codex": "none" }));
    let mut moved = scene.records.get("p_spoo");
    moved.report.logins = Some(vec!["codex/auth.json".into()]);
    scene.records.keep(moved);
    assert_eq!(words(&scene), json!({ "claude": "vault-key", "codex": "signed-in" }));
    scene.vault.borrow_mut().clear();
    assert_eq!(words(&scene), json!({ "claude": "none", "codex": "signed-in" }));
    let only = scene.ctx.sign_ins_here("p_spoo", Some(&["codex".to_string()]));
    assert_eq!(serde_json::to_value(only).unwrap(), json!({ "codex": "signed-in" }));
}

#[test]
fn emit_reaches_a_watcher_added_during_it_and_skips_one_removed() {
    let scene = Scene::new(SceneOpts::default());
    let order = Rc::new(RefCell::new(Vec::<&str>::new()));
    let ctx = Rc::clone(&scene.ctx);
    let late_off: Rc<RefCell<Option<Box<dyn FnOnce()>>>> = Rc::new(RefCell::new(None));
    let second_off: Rc<RefCell<Option<Box<dyn FnOnce()>>>> = Rc::new(RefCell::new(None));
    let (o1, c1, l1, s1) = (Rc::clone(&order), Rc::clone(&ctx), Rc::clone(&late_off), Rc::clone(&second_off));
    let _first = ctx.watch(Rc::new(move |_e: &PlaceEvent| {
        o1.borrow_mut().push("first");
        if let Some(off) = s1.borrow_mut().take() {
            off();
        }
        if l1.borrow().is_none() {
            let o = Rc::clone(&o1);
            *l1.borrow_mut() = Some(c1.watch(Rc::new(move |_e: &PlaceEvent| o.borrow_mut().push("added during"))));
        }
    }));
    let o2 = Rc::clone(&order);
    *second_off.borrow_mut() = Some(ctx.watch(Rc::new(move |_e: &PlaceEvent| o2.borrow_mut().push("removed before its turn"))));
    let absent: PlaceEvent = PlaceEvent::Absent(serde_json::from_value(json!({ "placeId": "p_spoo" })).unwrap());
    ctx.emit(&absent);
    assert_eq!(*order.borrow(), vec!["first", "added during"]);
    assert_eq!(scene.events.borrow().len(), 1, "the scene's own watcher, registered first, heard it once");
}

#[test]
#[should_panic(expected = "the place door is read before it is built")]
fn door_is_unset_until_the_door_is_built() {
    let scene = Scene::new(SceneOpts::default());
    let _ = scene.ctx.door();
}

#[test]
fn readers_ride_one_watch_and_each_sample_reaches_each_once() {
    block(async {
        let (_scene, socket, readings) = scene_with(CURRENT, answers_ok());
        let (page, page_got) = reader();
        let (pane, pane_got) = reader();
        let _a = readings.watch("p_spoo", page).await.expect("the page reads");
        let _b = readings.watch("p_spoo", pane).await.expect("the pane reads");
        assert_eq!(socket.reach.asked("sys.watch"), 1);
        heard(&readings, 1);
        heard(&readings, 2);
        assert_eq!(*page_got.borrow(), vec![sample(1), sample(2)]);
        assert_eq!(*pane_got.borrow(), vec![sample(1), sample(2)]);
    });
}

#[test]
fn the_last_reader_leaving_sends_one_unwatch() {
    block(async {
        let (_scene, socket, readings) = scene_with(CURRENT, answers_ok());
        let (page, _) = reader();
        let (pane, _) = reader();
        let page_off = readings.watch("p_spoo", page).await.unwrap();
        let pane_off = readings.watch("p_spoo", pane).await.unwrap();
        page_off();
        settle().await;
        assert_eq!(socket.reach.asked("sys.unwatch"), 0);
        pane_off();
        settle().await;
        assert_eq!(socket.reach.asked("sys.unwatch"), 1);
        let (again, again_got) = reader();
        let again_off = readings.watch("p_spoo", again).await.unwrap();
        assert_eq!(socket.reach.asked("sys.watch"), 2);
        heard(&readings, 3);
        assert_eq!(*again_got.borrow(), vec![sample(3)]);
        again_off();
        settle().await;
        assert_eq!(socket.reach.asked("sys.unwatch"), 2);
    });
}

#[test]
fn readers_are_held_while_away_and_asked_on_the_next_link() {
    block(async {
        let (scene, socket, readings) = scene_with(CURRENT, answers_ok());
        let (page, page_got) = reader();
        let _a = readings.watch("p_spoo", page).await.unwrap();
        heard(&readings, 1);
        scene.away("p_spoo", &socket);
        let (late, late_got) = reader();
        let _b = readings.watch("p_spoo", late).await.expect("a reader while away is held");
        let again = scene.live("p_spoo", answers_ok());
        readings.linked("p_spoo");
        settle().await;
        assert_eq!(again.reach.asked("sys.watch"), 1);
        assert_eq!(socket.reach.asked("sys.watch"), 1, "nothing was asked of the link that went");
        heard(&readings, 2);
        assert_eq!(*page_got.borrow(), vec![sample(1), sample(2)]);
        assert_eq!(*late_got.borrow(), vec![sample(2)]);
    });
}

#[test]
fn a_daemon_at_the_unwatch_version_is_sent_unwatch() {
    block(async {
        let (_scene, socket, readings) = scene_with(152, answers_ok());
        let (pane, _) = reader();
        let off = readings.watch("p_spoo", pane).await.unwrap();
        off();
        settle().await;
        assert_eq!(socket.reach.asked("sys.unwatch"), 1);
    });
}

#[test]
fn a_daemon_before_unwatch_is_never_asked_twice() {
    block(async {
        let (_scene, socket, readings) = scene_with(BEFORE_UNWATCH, answers_ok());
        let (first, _) = reader();
        let off = readings.watch("p_spoo", first).await.unwrap();
        off();
        settle().await;
        let (second, second_got) = reader();
        let off = readings.watch("p_spoo", second).await.unwrap();
        heard(&readings, 1);
        assert_eq!(*second_got.borrow(), vec![sample(1)]);
        off();
        let (page, page_got) = reader();
        let page_off = readings.watch("p_spoo", page).await.unwrap();
        page_off();
        let (page2, page2_got) = reader();
        let _keep = readings.watch("p_spoo", page2).await.unwrap();
        settle().await;
        assert_eq!([socket.reach.asked("sys.watch"), socket.reach.asked("sys.unwatch")], [1, 0]);
        heard(&readings, 2);
        assert_eq!(*page_got.borrow(), Vec::<SysSample>::new());
        assert_eq!(*page2_got.borrow(), vec![sample(2)]);
    });
}

#[test]
fn a_watch_in_flight_when_the_link_drops_stays_held() {
    block(async {
        let hangs = Rc::new(std::cell::Cell::new(true));
        let h = Rc::clone(&hangs);
        let script: Script = Rc::new(move |op, _| if op == "sys.watch" && h.get() { Reply::Hang } else { Reply::Ok(obj(json!({ "ok": true }))) });
        let (scene, socket, readings) = scene_with(CURRENT, script);
        let (page, page_got) = reader();
        let r = Rc::clone(&readings);
        let pending = tokio::task::spawn_local(async move { r.watch("p_spoo", page).await.map(|_| ()) });
        until("the watch went up", || socket.reach.asked("sys.watch") == 1).await;
        scene.away("p_spoo", &socket);
        assert!(pending.await.unwrap().is_ok(), "an answer lost with its link is no refusal");
        hangs.set(false);
        let again = scene.live("p_spoo", answers_ok());
        readings.linked("p_spoo");
        settle().await;
        assert_eq!(again.reach.asked("sys.watch"), 1);
        heard(&readings, 1);
        assert_eq!(*page_got.borrow(), vec![sample(1)]);
    });
}

#[test]
fn a_refusal_is_said_and_the_next_reader_asks_again() {
    block(async {
        let refusals = Rc::new(std::cell::Cell::new(1));
        let r = Rc::clone(&refusals);
        let script: Script = Rc::new(move |op, _| {
            if op == "sys.watch" && r.get() > 0 {
                r.set(r.get() - 1);
                Reply::Ok(obj(json!({ "ok": false, "error": "this computer reads no load" })))
            } else {
                Reply::Ok(obj(json!({ "ok": true })))
            }
        });
        let (_scene, socket, readings) = scene_with(CURRENT, script);
        let (page, _) = reader();
        let refused = readings.watch("p_spoo", page).await.err().expect("refused");
        assert_eq!(refused.message, "this computer reads no load");
        let (pane, pane_got) = reader();
        let _ok = readings.watch("p_spoo", pane).await.expect("the next reader asks again");
        assert_eq!(socket.reach.asked("sys.watch"), 2);
        heard(&readings, 1);
        assert_eq!(*pane_got.borrow(), vec![sample(1)]);
    });
}

#[test]
fn an_unknown_place_is_refused_with_the_places_there_are() {
    block(async {
        let (_scene, socket, readings) = scene_with(CURRENT, answers_ok());
        let (page, _) = reader();
        let refused = readings.watch("p_nowhere", page).await.err().expect("refused");
        assert_eq!(refused.message, no_such_place_refusal("p_nowhere", &["spoo".to_string()]));
        assert_eq!(socket.reach.frames.borrow().len(), 0);
    });
}

#[test]
fn a_refused_reader_is_released_and_leaves_no_watch() {
    block(async {
        let script: Script = Rc::new(|op, _| if op == "sys.watch" { Reply::Err("no load here".into()) } else { Reply::Ok(obj(json!({ "ok": true }))) });
        let (_scene, socket, readings) = scene_with(CURRENT, script);
        let (page, page_got) = reader();
        let refused = readings.watch("p_spoo", page).await.err().expect("a rejected frame on the live link is a refusal");
        assert_eq!(refused.message, "no load here");
        heard(&readings, 1);
        assert_eq!(*page_got.borrow(), Vec::<SysSample>::new());
        assert_eq!(socket.reach.asked("sys.unwatch"), 0);
    });
}

#[test]
fn a_second_release_does_nothing() {
    block(async {
        let (_scene, socket, readings) = scene_with(CURRENT, answers_ok());
        let (a, _) = reader();
        let (b, b_got) = reader();
        let off_a = readings.watch("p_spoo", a).await.unwrap();
        let _off_b = readings.watch("p_spoo", b).await.unwrap();
        off_a();
        off_a();
        settle().await;
        assert_eq!(socket.reach.asked("sys.unwatch"), 0);
        heard(&readings, 1);
        assert_eq!(*b_got.borrow(), vec![sample(1)]);
    });
}

#[test]
fn heard_passes_samples_alone_to_a_snapshot_of_readers() {
    block(async {
        let (_scene, _socket, readings) = scene_with(CURRENT, answers_ok());
        let got = Rc::new(RefCell::new(Vec::<&str>::new()));
        let second_off: Rc<RefCell<Option<Rc<dyn Fn()>>>> = Rc::new(RefCell::new(None));
        let (g1, s1) = (Rc::clone(&got), Rc::clone(&second_off));
        let _a = readings
            .watch(
                "p_spoo",
                Rc::new(move |_s: &SysSample| {
                    g1.borrow_mut().push("first");
                    if let Some(off) = s1.borrow().clone() {
                        off();
                    }
                }),
            )
            .await
            .unwrap();
        let g2 = Rc::clone(&got);
        let off = readings.watch("p_spoo", Rc::new(move |_s: &SysSample| g2.borrow_mut().push("second"))).await.unwrap();
        *second_off.borrow_mut() = Some(off);
        readings.heard("p_spoo", &serde_json::from_value(json!({ "type": "daemon.hello", "root": "/", "version": 1 })).unwrap());
        assert!(got.borrow().is_empty());
        heard(&readings, 1);
        assert_eq!(*got.borrow(), vec!["first", "second"]);
        heard(&readings, 2);
        assert_eq!(*got.borrow(), vec!["first", "second", "first"]);
    });
}

#[test]
fn a_release_after_the_link_was_replaced_sends_nothing() {
    block(async {
        let (scene, socket, readings) = scene_with(CURRENT, answers_ok());
        let (page, _) = reader();
        let off = readings.watch("p_spoo", page).await.unwrap();
        scene.away("p_spoo", &socket);
        let again = scene.live("p_spoo", answers_ok());
        off();
        settle().await;
        assert_eq!([socket.reach.asked("sys.unwatch"), again.reach.asked("sys.unwatch"), again.reach.asked("sys.watch")], [0, 0, 0]);
    });
}

#[test]
fn a_daemon_too_old_to_stop_keeps_its_watch_running() {
    block(async {
        let (scene, socket, readings) = scene_with(17, answers_ok());
        let (a, _) = reader();
        let off = readings.watch("p_spoo", a).await.unwrap();
        off();
        settle().await;
        assert_eq!(socket.reach.asked("sys.unwatch"), 0, "a daemon this old cannot be told to stop");
        let (b, _) = reader();
        let _keep = readings.watch("p_spoo", b).await.unwrap();
        assert_eq!(socket.reach.asked("sys.watch"), 1, "and the watch it still sends is ridden, not asked again");
        drop(scene);
    });
}
