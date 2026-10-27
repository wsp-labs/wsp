// SPDX-License-Identifier: AGPL-3.0-only
//! spec/context.md: the context handle of Proposal 2 and the state createRuntime starts it with.
mod support;

use std::panic::{catch_unwind, AssertUnwindSafe};
use std::rc::Rc;

use wsp_core::context::{Cancel, Joined, KindsArea, Late, RuntimeContext, RuntimeState, StateCell};
use wsp_wire::views::preferences::{preferences_from, Preferences};

#[test]
fn runtime_state_starts_as_create_runtime_fills_it() {
    let s = RuntimeState::default();
    assert!(s.say_stops.is_none());
    assert_eq!(s.reads_since, 0.0);
    assert!(s.roots_recheck.is_none());
    assert_eq!(s.owner, "");
    assert!(s.copies_moving.is_none());
    assert!(s.sweep_timer.is_none());
    assert!(s.sweeping.is_none());
    assert!(!s.sweep_stopped);
    assert!(s.beat.is_none());
    assert!(!s.closed);
    assert!(!s.closing);
    assert!(s.ticking.is_none());
    assert!(s.preferences_held.is_none());
    let ctx = RuntimeContext::new();
    assert_eq!(ctx.state.with(|s| (s.owner.clone(), s.closed, s.reads_since)), (String::new(), false, 0.0));
}

#[test]
fn with_hands_back_the_closures_value_and_clones_share_the_state() {
    let cell = StateCell::new(vec![1, 2]);
    let other = cell.clone();
    let len = cell.with(|v| {
        v.push(3);
        v.len()
    });
    assert_eq!(len, 3);
    assert_eq!(other.with(|v| v.clone()), vec![1, 2, 3]);
}

#[test]
fn with_inside_with_panics_as_a_double_borrow() {
    let cell = StateCell::new(0u32);
    let inner = cell.clone();
    let caught = catch_unwind(AssertUnwindSafe(|| cell.with(|_| inner.with(|n| *n))));
    assert!(caught.is_err(), "a borrow held while calling out is the mistake rule 3.3 forbids");
}

#[test]
fn a_panic_inside_with_leaves_the_cell_usable() {
    let cell = StateCell::new(String::from("before"));
    let caught = catch_unwind(AssertUnwindSafe(|| {
        cell.with(|s| {
            s.push_str(" half");
            panic!("a task failed mid-mutation");
        })
    }));
    assert!(caught.is_err());
    assert_eq!(cell.with(|s| s.clone()), "before half", "what the task half-changed stays half-changed");
}

#[test]
fn a_late_slot_read_before_it_is_filled_names_itself() {
    let slot: Late<Rc<u32>> = Late::named("kinds");
    let caught = catch_unwind(AssertUnwindSafe(|| slot.get()));
    let message = caught.unwrap_err();
    let text = message.downcast_ref::<String>().cloned().or_else(|| message.downcast_ref::<&str>().map(|s| s.to_string())).unwrap();
    assert_eq!(text, "the runtime read kinds before it was built");
}

#[test]
fn a_late_slot_filled_twice_panics() {
    let slot: Late<Rc<u32>> = Late::named("rules");
    slot.fill(Rc::new(1));
    let caught = catch_unwind(AssertUnwindSafe(|| slot.fill(Rc::new(2))));
    let message = caught.unwrap_err();
    let text = message.downcast_ref::<String>().cloned().or_else(|| message.downcast_ref::<&str>().map(|s| s.to_string())).unwrap();
    assert_eq!(text, "rules was built twice");
    assert_eq!(*slot.get(), 1);
}

#[test]
fn is_filled_reads_without_panicking() {
    let slot: Late<Rc<u32>> = Late::named("status");
    assert!(!slot.is_filled());
    slot.fill(Rc::new(7));
    assert!(slot.is_filled());
}

/// join A (#2476): the close empties each slot, so an area that holds the context that holds it is freed.
#[test]
fn a_cleared_late_slot_drops_what_it_held_and_reads_as_unbuilt() {
    let slot: Late<Rc<u32>> = Late::named("heldImages");
    let held = Rc::new(5);
    slot.fill(held.clone());
    assert_eq!(Rc::strong_count(&held), 2);
    slot.clear();
    assert_eq!(Rc::strong_count(&held), 1, "the slot let go");
    assert!(!slot.is_filled());
    let caught = catch_unwind(AssertUnwindSafe(|| slot.get()));
    let message = caught.unwrap_err();
    let text = message.downcast_ref::<String>().cloned().unwrap();
    assert_eq!(text, "the runtime read heldImages before it was built");
    slot.clear();
}

struct Kinds;
impl KindsArea for Kinds {}

#[test]
fn a_member_is_read_when_called_so_an_area_filled_later_is_reached() {
    let ctx = RuntimeContext::new();
    // An earlier area captured the context before the kinds area existed, as createRuntime builds them in file order.
    let captured = ctx.clone();
    let reaches = move || captured.kinds.is_filled();
    assert!(!reaches());
    let area: Rc<dyn KindsArea> = Rc::new(Kinds);
    ctx.kinds.fill(area.clone());
    assert!(reaches());
    assert!(Rc::ptr_eq(&ctx.kinds.get(), &area));
}

#[test]
fn a_partial_context_serves_the_slots_a_test_filled() {
    let ctx = RuntimeContext::new();
    ctx.kinds.fill(Rc::new(Kinds));
    assert!(ctx.kinds.is_filled());
    assert!(!ctx.rules.is_filled());
    assert!(!ctx.core.is_filled());
    let caught = catch_unwind(AssertUnwindSafe(|| ctx.threads.get()));
    assert!(caught.is_err());
}

// context.ts:174 types preferencesHeld as Preferences, which the preferences area and two other traits read.
#[test]
fn preferences_held_is_the_typed_record() {
    let ctx = RuntimeContext::new();
    let prefs: Preferences = preferences_from(None);
    ctx.state.with(|s| s.preferences_held = Some(prefs.clone()));
    let held: Option<Preferences> = ctx.state.with(|s| s.preferences_held.clone());
    assert_eq!(held, Some(prefs));
}

// The context names one Joined (the unit promise runtime state holds) and one Cancel (the clock's); the engine's and
// the machines' own stay where they are.
#[test]
fn the_context_names_its_joined_and_its_cancel() {
    let joined: Joined = futures::FutureExt::shared(futures::FutureExt::boxed_local(async {}));
    let cancel: Cancel = Rc::new(|| {});
    let same_cancel: wsp_core::clock::Cancel = cancel.clone();
    let ctx = RuntimeContext::new();
    ctx.state.with(|s| {
        s.ticking = Some(joined);
        s.sweep_timer = Some(same_cancel);
    });
    assert!(ctx.state.with(|s| s.ticking.is_some() && s.sweep_timer.is_some()));
}
