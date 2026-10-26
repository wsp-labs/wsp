// SPDX-License-Identifier: AGPL-3.0-only
//! Pins places.ts, the door's assembly (`makePlaceDoor`, :38-46): one context, the halves each on their own methods, the
//! door set on the context, the watchers the context emits to. Spec: spec/places_door.md.
mod common;

use std::cell::RefCell;
use std::rc::Rc;

use wsp_core::types::pending::{Clock, Store};
use wsp_machines::places::types::{
    HerePlace, PlaceDoor, PlaceDoorOptions, PlaceEvent, PlaceRecording, PlaceWiring,
};
use wsp_machines::places::views::LinkReads;
use wsp_machines::places::{
    assemble_place_door, make_place_door, AssembledDoor, DoorLinkReads, LINK_HALF, MANAGE_HALF,
};
use wsp_store::keys::{key_fingerprint, new_place_key_pair, PlaceKeyPair};
use wsp_wire::views::place_more::PlaceRemovedEvent;

use common::places::*;

/// Options over the fakes with no road wired and no wait pinned, and the host key the door must answer for.
fn options() -> (PlaceDoorOptions, PlaceKeyPair) {
    let host_key = new_place_key_pair();
    let wiring = PlaceWiring {
        host_key: host_key.clone(),
        provider: None,
        here: Rc::new(|| HerePlace {
            name: "zingzys-mac".into(),
            ..Default::default()
        }),
        host_name: Rc::new(|| "zingzys-mac".to_string()),
        install: None,
        dial: None,
        log: None,
        update: None,
        leave: None,
        forget_key: None,
        sudo_over: None,
        provision: None,
        github_token: None,
        undo: None,
        run_over: None,
        back: None,
    };
    let opts = PlaceDoorOptions {
        store: Rc::new(FakeStore::default()) as Rc<dyn Store>,
        devices: Rc::new(FakeDevices),
        wiring,
        recording: Rc::new(FakeRecording::default()) as Rc<dyn PlaceRecording>,
        providers: None,
        on_daemon_event: None,
        on_stage: None,
        on_setup: None,
        copy_build: None,
        nap_ms: None,
        here_daemon: None,
        nap_changed: None,
        join_wait_ms: None,
        update_wait_ms: None,
        dial_wait_ms: None,
        frame_wait_ms: None,
        relink_wait_ms: None,
        seen_every_ms: None,
        vault: None,
        recipes: None,
        owner: None,
        kill_confirm: None,
        clock: Some(TestClock::new() as Rc<dyn Clock>),
    };
    (opts, host_key)
}

fn assembled() -> (Rc<AssembledDoor>, PlaceKeyPair) {
    let (opts, key) = options();
    (assemble_place_door(opts), key)
}

#[test]
fn every_area_and_both_halves_hold_the_one_context() {
    let (door, _) = assembled();
    assert!(Rc::ptr_eq(&door.ctx, &door.link.ctx));
    assert!(Rc::ptr_eq(&door.ctx, &door.manage.ctx));
    assert!(Rc::ptr_eq(&door.ctx, &door.manage.records.ctx));
    assert!(Rc::ptr_eq(&door.ctx, &door.manage.views.ctx));
    assert!(Rc::ptr_eq(&door.ctx, &door.link.setup.ctx));
}

#[test]
fn the_door_is_set_on_the_context_once_the_assembly_is_done() {
    let (door, _) = assembled();
    let held = door.ctx.door();
    assert!(std::ptr::addr_eq(Rc::as_ptr(&held), Rc::as_ptr(&door)));
}

#[test]
fn the_two_halves_name_every_door_method_once() {
    assert_eq!(LINK_HALF.len(), 33);
    assert_eq!(MANAGE_HALF.len(), 28);
    for name in LINK_HALF {
        assert!(!MANAGE_HALF.contains(&name), "{name} is on both halves");
    }
    let mut all: Vec<&str> = LINK_HALF
        .iter()
        .chain(MANAGE_HALF.iter())
        .copied()
        .collect();
    all.sort_unstable();
    all.dedup();
    assert_eq!(all.len(), 61, "a name appears twice or a method is missing");
}

#[test]
fn a_link_method_is_answered_by_the_link_half() {
    let (door, key) = assembled();
    assert_eq!(door.host_key(), key_fingerprint(&key.public_key));
    assert_eq!(door.host_key(), door.link.host_key());
}

#[test]
fn a_manage_method_is_answered_by_the_manage_half() {
    let (door, _) = assembled();
    assert!(door.adds().is_empty());
    assert_eq!(door.adds(), door.manage.adds());
}

#[test]
fn a_watcher_the_door_holds_hears_what_the_context_emits_until_it_is_taken_off() {
    let (door, _) = assembled();
    let heard: Rc<RefCell<Vec<PlaceEvent>>> = Rc::new(RefCell::new(Vec::new()));
    let sink = Rc::clone(&heard);
    let id = door.on(Rc::new(move |e: &PlaceEvent| {
        sink.borrow_mut().push(e.clone())
    }));
    let event = PlaceEvent::Removed(PlaceRemovedEvent {
        place_id: "p_spoo".to_string(),
    });
    door.ctx.emit(&event);
    assert_eq!(heard.borrow().len(), 1);
    door.off(id);
    door.ctx.emit(&event);
    assert_eq!(heard.borrow().len(), 1);
}

#[test]
fn views_read_the_link_half_through_the_door_at_the_call() {
    let (door, _) = assembled();
    let reads = DoorLinkReads {
        ctx: Rc::clone(&door.ctx),
    };
    assert!(reads.backend_of("p_none").is_none());
    assert_eq!(
        reads.backend_of("p_none").is_none(),
        door.backend_of("p_none").is_none()
    );
    assert!(reads.folder_computer("p_none").is_none());
}

#[test]
fn make_place_door_answers_the_assembled_door() {
    let (opts, key) = options();
    let door = make_place_door(opts);
    assert_eq!(door.host_key(), key_fingerprint(&key.public_key));
    assert!(door.adds().is_empty());
}
