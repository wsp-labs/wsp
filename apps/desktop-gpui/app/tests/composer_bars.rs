// SPDX-License-Identifier: AGPL-3.0-only
//! The bars a drawer row opens in the composer's place (Dock.tsx, bars/TasksBar.tsx, bars/UsageBar.tsx,
//! bars/CapBar.tsx): each in the dock's frame, its acts, Esc and Write a message folding it, and the bar going with
//! what it says.

#[path = "composer/harness.rs"]
mod harness;
#[path = "composer/states.rs"]
mod states;

use std::cell::RefCell;
use std::rc::Rc;

use desktop_gpui::screens::composer::model::{Drawer, Limit, Plan, Slot};
use desktop_gpui::screens::composer::{Bar, Composer, ComposerEvent};
use gpui_kit::{px, Entity, Focusable, Modifiers, TestAppContext, VisualTestContext};
use harness::{composer_of, open};
use serde_json::json;

fn press(cx: &mut VisualTestContext, selector: &'static str) {
    let at = cx.debug_bounds(selector).unwrap_or_else(|| panic!("nothing drew {selector}")).center();
    cx.simulate_click(at, Modifiers::none());
    cx.run_until_parked();
}

fn marks(composer: &Entity<Composer>, cx: &mut VisualTestContext) -> Rc<RefCell<Vec<bool>>> {
    let seen = Rc::new(RefCell::new(Vec::new()));
    let sink = seen.clone();
    cx.update(|_, cx| {
        cx.subscribe(composer, move |_, event: &ComposerEvent, _| {
            if let ComposerEvent::Mark(resume) = event {
                sink.borrow_mut().push(*resume);
            }
        })
        .detach()
    });
    seen
}

/// Three steps: the first set working at 1 s and done at 4 s, the second at work since 4 s, the third still to do.
fn plan() -> Plan {
    let step = |text: &str, state: &str| json!({ "text": text, "state": state });
    let first = [step("Read the list code", "working"), step("Pin the anchor", "pending"), step("Write the test", "pending")];
    let earlier = Plan::next(None, &first, Some(1_000.));
    let then = [step("Read the list code", "done"), step("Pin the anchor", "working"), step("Write the test", "pending")];
    Plan::next(Some(&earlier), &then, Some(4_000.))
}

#[gpui_kit::test]
fn the_tasks_row_opens_the_steps_with_their_times_in_the_docks_frame(cx: &mut TestAppContext) {
    let cx = open("running", cx);
    let composer = composer_of(cx);
    let tasks = plan().tasks().expect("a step is left to work on");
    assert_eq!((tasks.step.as_str(), tasks.count()), ("Pin the anchor", "1/3".to_owned()));
    let times: Vec<(Option<u64>, Option<f64>)> = tasks.steps.iter().map(|s| (s.took, s.since)).collect();
    assert_eq!(times, [(Some(3_000), None), (None, Some(4_000.)), (None, None)], "a done step took 3 s, the one at work since 4 s");
    composer.update(cx, |c, cx| c.set_drawer(Drawer { tasks: Some(tasks), ..Drawer::default() }, cx));
    cx.run_until_parked();
    press(cx, "drawer-tasks");
    assert_eq!(composer.read_with(cx, |c, _| c.bar()), Some(Bar::Tasks));
    assert!(cx.debug_bounds("composer-tasks-bar").is_some(), "the Tasks bar stands in the composer's place");
    assert!(cx.debug_bounds("dock-aside").is_some(), "done of all at the head's right");
    assert!(cx.debug_bounds("step-time").is_some(), "a step's time at its right");
    assert!(cx.debug_bounds("composer-editor").is_none(), "no box under the bar");

    // Dock.tsx's frame at 1024 px: the box in the 768 px column, the title 24 px high, 20 px under the foot.
    let boxed = cx.debug_bounds("composer-box").unwrap();
    assert_eq!((boxed.left(), boxed.size.width), (px(128.), px(768.)));
    assert_eq!(cx.debug_bounds("dock-title").unwrap().size.height, px(24.), "a 16 px semibold title on 24 px");
    let foot = cx.debug_bounds("dock-foot").unwrap();
    assert_eq!(boxed.bottom() - foot.bottom(), px(1.), "the foot ends at the hairline");
    assert_eq!(foot.bottom() - cx.debug_bounds("dock-back").unwrap().bottom(), px(20.), "`pb-5` under Write a message");
    assert_eq!(boxed.bottom() + px(16.), cx.debug_bounds("composer").unwrap().bottom(), "16 px round the dock");

    // The bar goes with what it says.
    composer.update(cx, |c, cx| c.set_drawer(Drawer::default(), cx));
    cx.run_until_parked();
    assert_eq!(composer.read_with(cx, |c, _| c.bar()), None);
    assert!(cx.debug_bounds("composer-editor").is_some(), "the box stands again");
}

#[gpui_kit::test]
fn esc_and_write_a_message_fold_a_bar_back_to_the_box(cx: &mut TestAppContext) {
    let cx = open("running", cx);
    let composer = composer_of(cx);
    composer.update(cx, |c, cx| c.set_drawer(Drawer { tasks: plan().tasks(), ..Drawer::default() }, cx));
    composer.update(cx, |c, cx| c.open_bar(Bar::Tasks, cx));
    cx.run_until_parked();
    cx.simulate_keystrokes("escape");
    cx.run_until_parked();
    assert_eq!(composer.read_with(cx, |c, _| c.bar()), None, "Esc folds the bar");
    assert!(cx.update(|window, cx| composer.read(cx).editor().focus_handle(cx).is_focused(window)), "the box has the keys");
    composer.update(cx, |c, cx| c.open_bar(Bar::Tasks, cx));
    cx.run_until_parked();
    press(cx, "dock-back");
    assert_eq!(composer.read_with(cx, |c, _| c.bar()), None, "Write a message folds it too");
}

#[gpui_kit::test]
fn the_usage_bar_arms_resume_at_reset_and_cancel_takes_it_back(cx: &mut TestAppContext) {
    let cx = open("empty", cx);
    let composer = composer_of(cx);
    let marked = marks(&composer, cx);
    let limit = |armed: bool, resets: bool| Limit {
        title: if armed { "Resumes at the reset" } else { "Usage limit reached" }.into(),
        line: "Claude Code resumes at 18:00".into(),
        note: "Claude Code stopped this turn until its plan resets.".into(),
        resets,
        armed,
    };
    composer.update(cx, |c, cx| c.set_drawer(Drawer { limit: Some(limit(false, true)), ..Drawer::default() }, cx));
    cx.run_until_parked();
    press(cx, "drawer-usage");
    assert!(cx.debug_bounds("composer-usage-bar").is_some());
    assert!(cx.debug_bounds("dock-note").is_some(), "the agent's sentence under the title");
    press(cx, "limit-resume");
    assert_eq!(*marked.borrow(), [true], "Resume at reset marks the thread");
    assert_eq!(composer.read_with(cx, |c, _| c.bar()), None, "and folds the bar");

    // Not armed, Cancel only folds; armed, it takes the mark back and Resume is held.
    composer.update(cx, |c, cx| c.open_bar(Bar::Usage, cx));
    cx.run_until_parked();
    press(cx, "limit-cancel");
    assert_eq!(*marked.borrow(), [true]);
    assert_eq!(composer.read_with(cx, |c, _| c.bar()), None);
    composer.update(cx, |c, cx| {
        c.set_drawer(Drawer { limit: Some(limit(true, true)), ..Drawer::default() }, cx);
        c.open_bar(Bar::Usage, cx);
    });
    cx.run_until_parked();
    press(cx, "limit-resume");
    assert_eq!(*marked.borrow(), [true], "a held Resume does nothing");
    press(cx, "limit-cancel");
    assert_eq!(*marked.borrow(), [true, false], "Cancel takes the armed resume back");

    // No reset named: the bar holds its words alone.
    composer.update(cx, |c, cx| {
        c.set_drawer(Drawer { limit: Some(limit(false, false)), ..Drawer::default() }, cx);
        c.open_bar(Bar::Usage, cx);
    });
    cx.run_until_parked();
    assert!(cx.debug_bounds("composer-usage-bar").is_some());
    assert!(cx.debug_bounds("limit-resume").is_none() && cx.debug_bounds("limit-cancel").is_none(), "no acts");
}

#[gpui_kit::test]
fn the_cap_bar_names_the_computer_and_offers_its_setting(cx: &mut TestAppContext) {
    let cx = open("empty", cx);
    let composer = composer_of(cx);
    let slot = |raise: Option<&str>| Slot {
        line: "build-box is running 2 of 2 threads".into(),
        title: "Waiting for a thread slot on build-box".into(),
        note: "2 of 2 threads are running. It starts on its own when one ends.".into(),
        raise: raise.map(str::to_owned),
        place_id: "p1".into(),
    };
    composer.update(cx, |c, cx| c.set_drawer(Drawer { slot: Some(slot(Some("Raise threads at once"))), ..Drawer::default() }, cx));
    cx.run_until_parked();
    press(cx, "drawer-capped");
    assert!(cx.debug_bounds("composer-cap-bar").is_some());
    assert!(cx.debug_bounds("cap-raise").is_some(), "the setting that lets it start now");
    composer.update(cx, |c, cx| c.set_drawer(Drawer { slot: Some(slot(None)), ..Drawer::default() }, cx));
    cx.run_until_parked();
    assert!(cx.debug_bounds("cap-raise").is_none(), "a minute of starts has no setting to move");
}

/// A bar that goes on its own, its turn ended or the thread switched, hands the keys back to the box.
#[gpui_kit::test]
fn a_bar_that_goes_on_its_own_gives_the_box_the_keys(cx: &mut TestAppContext) {
    let cx = open("running", cx);
    let composer = composer_of(cx);
    composer.update_in(cx, |c, window, cx| c.set_thread_key("thread-1".into(), window, cx));
    let boxed = |cx: &mut VisualTestContext| cx.update(|window, cx| composer.read(cx).editor().focus_handle(cx).is_focused(window));
    composer.update(cx, |c, cx| {
        c.set_drawer(Drawer { tasks: plan().tasks(), ..Drawer::default() }, cx);
        c.open_bar(Bar::Tasks, cx);
    });
    cx.run_until_parked();
    assert!(!boxed(cx), "the bar holds the keys");
    composer.update(cx, |c, cx| c.set_drawer(Drawer::default(), cx));
    cx.run_until_parked();
    assert!(boxed(cx), "the steps gone, the box has the keys again");

    composer.update(cx, |c, cx| {
        c.set_drawer(Drawer { tasks: plan().tasks(), ..Drawer::default() }, cx);
        c.open_bar(Bar::Tasks, cx);
    });
    cx.run_until_parked();
    composer.update_in(cx, |c, window, cx| c.set_thread_key("thread-2".into(), window, cx));
    cx.run_until_parked();
    assert!(boxed(cx), "a switch hands the box the keys");
}

/// Under a capture's fixed now the step at work counts from that now, as the thread's working line does, not from the
/// wall clock: one second in reads 1s, thirty-six in reads 36s (today's capture fixes `Date.now` for the whole page).
#[gpui_kit::test]
fn the_step_at_work_counts_from_a_captures_fixed_now(cx: &mut TestAppContext) {
    use desktop_gpui::ui::FrozenNow;
    let cx = open("running", cx);
    let composer = composer_of(cx);
    let at = |now: f64, cx: &mut VisualTestContext| {
        cx.update(|window, cx| {
            cx.set_global(FrozenNow(now));
            window.refresh();
        });
        cx.run_until_parked();
        cx.debug_bounds("step-time").expect("the step at work's clock").size.width
    };
    composer.update(cx, |c, cx| {
        c.set_drawer(Drawer { tasks: plan().tasks(), ..Drawer::default() }, cx);
        c.open_bar(Bar::Tasks, cx);
    });
    cx.run_until_parked();
    // The step at work began at 4 s.
    let (one, thirty_six) = (at(5_000., cx), at(40_000., cx));
    assert!(thirty_six > one, "the clock read the wall clock, not the fixed now: 1s is {one:?}, 36s is {thirty_six:?}");
}
