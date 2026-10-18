// SPDX-License-Identifier: AGPL-3.0-only
//! What the composer does with keys, menus and the store, against the web app's rules: Up and Down through the sent
//! prompts, the `#` and `$` menus, Cmd or Ctrl and a digit, the favourites, Fast mode, a thread pinned to its agent,
//! the drawer and the queue's bar, a long paste, a key typed outside any field, and what the store keeps.

#[path = "composer/harness.rs"]
mod harness;
#[path = "composer/states.rs"]
mod states;

use desktop_gpui::local_store;
use desktop_gpui::screens::composer::model::{Command, Drawer, HostItem, Skill};
use desktop_gpui::screens::composer::screen::types_into_composer;
use desktop_gpui::screens::composer::{store, Bar, Popup};
use gpui_kit::{ClipboardItem, Focusable, KeyDownEvent, Keystroke, TestAppContext, VisualContext as _};
use harness::{composer_of, open, starts};

fn prompts(list: &[&str]) -> Vec<(String, String)> {
    list.iter().enumerate().map(|(i, p)| (i.to_string(), (*p).to_owned())).collect()
}

#[gpui_kit::test]
fn up_and_down_walk_the_threads_sent_prompts(cx: &mut TestAppContext) {
    let cx = open("empty", cx);
    let composer = composer_of(cx);
    composer.update_in(cx, |c, window, cx| {
        c.set_recalls(prompts(&["first ask", "second ask"]), cx);
        c.editor().focus_handle(cx).focus(window, cx);
    });
    let text = |cx: &mut gpui_kit::VisualTestContext| composer.read_with(cx, |c, cx| c.text(cx));
    cx.simulate_keystrokes("up");
    assert_eq!(text(cx), "second ask");
    cx.simulate_keystrokes("up");
    assert_eq!(text(cx), "first ask");
    cx.simulate_keystrokes("down");
    assert_eq!(text(cx), "second ask");
    cx.simulate_keystrokes("down");
    assert_eq!(text(cx), "", "past the newest the box empties");
    cx.simulate_input("a draft");
    cx.simulate_keystrokes("up");
    assert_eq!(text(cx), "a draft", "a draft keeps Up for its caret");
}

#[gpui_kit::test]
fn the_hash_menu_asks_for_the_repositorys_items_and_a_pick_is_a_chip(cx: &mut TestAppContext) {
    let cx = open("empty", cx);
    let composer = composer_of(cx);
    let asked = std::rc::Rc::new(std::cell::Cell::new(0));
    let count = asked.clone();
    cx.update(|_, cx| {
        cx.subscribe(&composer, move |_, e: &desktop_gpui::screens::composer::ComposerEvent, _| {
            if matches!(e, desktop_gpui::screens::composer::ComposerEvent::Items) {
                count.set(count.get() + 1);
            }
        })
        .detach()
    });
    composer.update_in(cx, |c, window, cx| {
        c.editor().focus_handle(cx).focus(window, cx);
        c.set_text("see #", window, cx);
    });
    assert_eq!(asked.get(), 1, "the first # asks the host once");
    let item = HostItem {
        kind: "pull-request".into(),
        number: 12,
        title: "Fix the scroll".into(),
        body: "Pins the anchor.".into(),
        url: "https://example.com/12".into(),
    };
    composer.update_in(cx, |c, _, cx| c.set_host_items(vec![item], None, cx));
    let labels = composer.read_with(cx, |c, _| c.menu_items().unwrap().iter().map(|i| i.label.clone()).collect::<Vec<_>>());
    assert_eq!(labels, ["#12 Fix the scroll"]);
    cx.simulate_keystrokes("enter");
    assert_eq!(composer.read_with(cx, |c, cx| c.editor().read(cx).tokens().len()), 1, "the item goes as one chip");
    let started = starts(&composer, cx);
    cx.simulate_keystrokes("enter");
    assert_eq!(
        *started.borrow(),
        ["see Pull request #12 \"Fix the scroll\" (https://example.com/12):\n> Pins the anchor."],
        "the send carries the quoted item"
    );
}

#[gpui_kit::test]
fn the_dollar_menu_lists_the_agents_skills(cx: &mut TestAppContext) {
    let cx = open("empty", cx);
    let composer = composer_of(cx);
    composer.update_in(cx, |c, window, cx| {
        c.set_skills(vec![Skill { name: "tidy-imports".into(), description: Some("Sort the imports".into()) }], cx);
        c.editor().focus_handle(cx).focus(window, cx);
    });
    cx.simulate_input("$ti");
    let labels = composer.read_with(cx, |c, _| c.menu_items().unwrap().iter().map(|i| i.label.clone()).collect::<Vec<_>>());
    assert_eq!(labels, ["$tidy-imports"]);
    cx.simulate_keystrokes("enter");
    assert_eq!(composer.read_with(cx, |c, cx| c.text(cx)), "$tidy-imports ");
    cx.simulate_input("/");
    composer.update_in(cx, |c, window, cx| c.set_text("/", window, cx));
    let groups = composer.read_with(cx, |c, _| c.menu_items().unwrap().iter().map(|i| i.group.clone()).collect::<Vec<_>>());
    assert!(groups.contains(&"Skills".to_owned()), "the slash menu lists the skills under Skills: {groups:?}");
}

#[gpui_kit::test]
fn a_digit_with_the_command_key_picks_that_row_of_the_model_picker(cx: &mut TestAppContext) {
    let cx = open("model-picker-open", cx);
    let composer = composer_of(cx);
    cx.simulate_keystrokes("secondary-2");
    composer.read_with(cx, |c, _| {
        assert_eq!(c.popup(), None);
        assert_eq!(c.picks().model.as_deref(), Some("claude-fable-5-1"));
    });
}

#[gpui_kit::test]
fn a_starred_model_shows_on_the_favourites_tab(cx: &mut TestAppContext) {
    let cx = open("model-picker-open", cx);
    let composer = composer_of(cx);
    composer.update(cx, |c, cx| {
        c.toggle_favourite("claude:claude-haiku-5-5".into(), cx);
        c.show_favourites(cx);
    });
    assert_eq!(composer.read_with(cx, |c, cx| c.popup_rows(cx)), ["claude:claude-haiku-5-5"]);
    cx.simulate_keystrokes("secondary-1");
    composer.read_with(cx, |c, _| {
        assert_eq!(c.picks().model.as_deref(), Some("claude-haiku-5-5"));
        assert_eq!(c.picks().agent.as_deref(), Some("claude"));
    });
    composer.update(cx, |c, cx| c.toggle_favourite("claude:claude-haiku-5-5".into(), cx));
    assert!(composer.read_with(cx, |c, _| c.favourites().is_empty()), "a second press unstars it");
}

#[gpui_kit::test]
fn fast_mode_is_offered_where_the_model_has_it_and_goes_with_the_send(cx: &mut TestAppContext) {
    let cx = open("new-thread", cx);
    let composer = composer_of(cx);
    let started = std::rc::Rc::new(std::cell::RefCell::new(Vec::new()));
    let sink = started.clone();
    cx.update(|_, cx| {
        cx.subscribe(&composer, move |_, e: &desktop_gpui::screens::composer::ComposerEvent, _| {
            if let desktop_gpui::screens::composer::ComposerEvent::Start(out) = e {
                sink.borrow_mut().push(out.fast);
            }
        })
        .detach()
    });
    composer.update(cx, |c, cx| c.toggle_popup(Popup::Reasoning, cx));
    let rows = composer.read_with(cx, |c, cx| c.popup_rows(cx));
    assert!(rows.ends_with(&["fast:on".to_owned(), "fast:off".to_owned()]), "Opus 5.5 offers Fast mode: {rows:?}");
    composer.update_in(cx, |c, window, cx| {
        c.pick(Popup::Reasoning, "fast:on".into(), window, cx);
        c.set_text("quick one", window, cx);
        c.submit(window, cx);
    });
    assert_eq!(*started.borrow(), [true]);
}

#[gpui_kit::test]
fn a_thread_that_has_run_offers_only_its_own_agents_models(cx: &mut TestAppContext) {
    let cx = open("model-picker-open", cx);
    let composer = composer_of(cx);
    let before = composer.read_with(cx, |c, cx| c.popup_rows(cx));
    composer.update(cx, |c, cx| c.set_rail("codex".into(), cx));
    assert_eq!(composer.read_with(cx, |c, cx| c.popup_rows(cx)), before, "the rows stay claude's");
    assert!(cx.debug_bounds("composer-model-foot").is_some(), "the foot says a new thread is needed");
    composer.update_in(cx, |c, window, cx| c.pick(Popup::Model, "gpt-5.6-sol".into(), window, cx));
    assert_eq!(composer.read_with(cx, |c, _| c.picks().model.clone()), None, "another agent's model is not picked");
}

#[gpui_kit::test]
fn the_drawer_shows_each_waiting_row_and_the_queue_opens_its_bar(cx: &mut TestAppContext) {
    let cx = open("queued", cx);
    let composer = composer_of(cx);
    composer.update(cx, |c, cx| {
        c.set_drawer(
            Drawer {
                question: Some("Run cargo test?".into()),
                limit: Some(("Usage limit reached".into(), "Claude Code resumes at 18:00".into())),
                slot: Some("build-box is running 2 of 2 threads".into()),
                tasks: Some(("Write the test".into(), "1/3".into())),
            },
            cx,
        )
    });
    cx.run_until_parked();
    for row in ["drawer-question", "drawer-usage", "drawer-capped", "drawer-tasks", "drawer-queue"] {
        assert!(cx.debug_bounds(row).is_some(), "{row} is drawn");
    }
    composer.update(cx, |c, cx| c.open_bar(Bar::Queue, cx));
    cx.run_until_parked();
    assert!(cx.debug_bounds("composer-queue-bar").is_some());
    let id = composer.read_with(cx, |c, _| c.queue()[0].id);
    composer.update_in(cx, |c, window, cx| c.edit_queued(id, window, cx));
    composer.read_with(cx, |c, cx| {
        assert_eq!(c.text(cx), "Then check the scroll anchor too.", "Edit puts the words back in the box");
        assert!(c.queue().is_empty());
        assert_eq!(c.bar(), None, "and folds the bar");
    });
}

#[gpui_kit::test]
fn a_slates_words_fold_the_bar_and_the_box_takes_the_keys(cx: &mut TestAppContext) {
    let cx = open("queued", cx);
    let composer = composer_of(cx);
    composer.update_in(cx, |c, window, cx| c.set_thread_key("thread-1".into(), window, cx));
    composer.update(cx, |c, cx| c.open_bar(Bar::Queue, cx));
    assert!(cx.update(|window, cx| desktop_gpui::screens::composer::fill("thread-1", "Build a slate", window, cx)));
    composer.read_with(cx, |c, cx| {
        assert_eq!(c.text(cx), "Build a slate");
        assert_eq!(c.editor().read(cx).cursor(), "Build a slate".len(), "the caret after the words");
        assert_eq!(c.bar(), None, "Ask for one folds the bar");
    });
    assert!(cx.update(|window, cx| composer.read(cx).editor().focus_handle(cx).is_focused(window)), "the box has the keys");
    composer.update(cx, |c, cx| c.open_bar(Bar::Queue, cx));
    assert!(cx.update(|window, cx| desktop_gpui::screens::composer::insert("thread-1", "> a: 1", window, cx)));
    composer.read_with(cx, |c, cx| {
        assert_eq!(c.text(cx), "Build a slate\n> a: 1\n", "a fill's block goes in at the caret on lines of its own");
        assert_eq!(c.bar(), None, "and folds the bar too");
    });
    assert!(!cx.update(|window, cx| desktop_gpui::screens::composer::fill("thread-3", "x", window, cx)), "no composer has thread-3");
}

#[gpui_kit::test]
fn a_long_paste_goes_as_a_text_file(cx: &mut TestAppContext) {
    let cx = open("empty", cx);
    let composer = composer_of(cx);
    composer.update_in(cx, |c, window, cx| c.editor().focus_handle(cx).focus(window, cx));
    cx.write_to_clipboard(ClipboardItem::new_string("a line\n".repeat(250)));
    cx.simulate_keystrokes("secondary-v");
    composer.read_with(cx, |c, cx| {
        assert_eq!(c.files().iter().map(|f| f.name.as_str()).collect::<Vec<_>>(), ["pasted-text-1.txt"]);
        assert_eq!(c.text(cx), "", "the text does not land in the box");
    });
    cx.write_to_clipboard(ClipboardItem::new_string("short".into()));
    cx.simulate_keystrokes("secondary-v");
    assert_eq!(composer.read_with(cx, |c, cx| c.text(cx)), "short", "a short paste is text");
}

#[gpui_kit::test]
fn a_key_typed_outside_any_field_lands_in_the_box(cx: &mut TestAppContext) {
    let cx = open("queued", cx);
    let composer = composer_of(cx);
    let key = |k: &str, typed: Option<&str>| {
        let mut keystroke = Keystroke::parse(k).unwrap();
        keystroke.key_char = typed.map(str::to_owned);
        KeyDownEvent { keystroke, is_held: false, prefer_character_input: false }
    };
    assert_eq!(types_into_composer(&key("h", Some("h"))), Some("h"));
    assert_eq!(types_into_composer(&key("cmd-h", Some("h"))), None);
    assert_eq!(types_into_composer(&key("space", Some(" "))), None);
    assert_eq!(types_into_composer(&key("left", None)), None);
    composer.update(cx, |c, cx| c.open_bar(Bar::Queue, cx));
    composer.update_in(cx, |c, window, cx| c.type_key("h", window, cx));
    composer.read_with(cx, |c, cx| {
        assert_eq!(c.text(cx), "h");
        assert_eq!(c.bar(), None, "a bar folds for the key");
    });
    assert!(cx.update(|window, cx| composer.read(cx).editor().focus_handle(cx).is_focused(window)), "the box has the keys");
}

#[gpui_kit::test]
fn the_store_keeps_the_queue_held_the_stash_and_the_draft(cx: &mut TestAppContext) {
    let dir = std::env::temp_dir().join(format!("composer-keys-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    cx.update(|cx| local_store::init(dir.join("local-store.json"), cx));
    let cx = open("running", cx);
    let composer = composer_of(cx);
    composer.update_in(cx, |c, window, cx| {
        c.set_thread_key("thread-1".into(), window, cx);
        c.set_text("Then the sidebar.", window, cx);
        c.submit(window, cx);
        c.set_text("stash me", window, cx);
        c.stash_draft(window, cx);
        c.set_text("a draft left open", window, cx);
    });
    cx.executor().advance_clock(std::time::Duration::from_secs(1));
    cx.run_until_parked();

    // A second composer reads back what the first left, as the app does on its next launch, off the file.
    cx.update(|_, cx| local_store::init(dir.join("local-store.json"), cx));
    let again = cx.new_window_entity(desktop_gpui::screens::composer::Composer::new);
    let started = starts(&again, cx);
    again.update_in(cx, |c, window, cx| {
        c.set_turn(true, true, cx);
        c.set_thread_key("thread-1".into(), window, cx);
    });
    again.read_with(cx, |c, cx| {
        assert_eq!(c.text(cx), "a draft left open");
        assert_eq!(c.queue().iter().map(|q| q.prompt.as_str()).collect::<Vec<_>>(), ["Then the sidebar."]);
        assert_eq!(c.stash().entries.iter().map(|e| e.prompt.as_str()).collect::<Vec<_>>(), ["stash me"]);
    });
    again.update(cx, |c, cx| c.set_turn(true, false, cx));
    assert!(started.borrow().is_empty(), "a queue read back waits for the next send");
    let _ = std::fs::remove_dir_all(&dir);
}

#[gpui_kit::test]
fn a_send_and_a_stash_empty_the_kept_draft(cx: &mut TestAppContext) {
    let dir = std::env::temp_dir().join(format!("composer-draft-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    cx.update(|cx| local_store::init(dir.join("local-store.json"), cx));
    let cx = open("empty", cx);
    let composer = composer_of(cx);
    let kept = |cx: &mut gpui_kit::VisualTestContext| cx.update(|_, cx| store::draft(cx.global::<local_store::LocalStore>(), "thread-1"));
    composer.update_in(cx, |c, window, cx| {
        c.set_thread_key("thread-1".into(), window, cx);
        c.set_text("fix it", window, cx);
    });
    cx.executor().advance_clock(std::time::Duration::from_secs(1));
    cx.run_until_parked();
    assert_eq!(kept(cx).map(|(t, _)| t).as_deref(), Some("fix it"));
    composer.update_in(cx, |c, window, cx| c.submit(window, cx));
    cx.executor().advance_clock(std::time::Duration::from_secs(1));
    cx.run_until_parked();
    assert_eq!(kept(cx), None, "a sent draft is not kept");
    composer.update_in(cx, |c, window, cx| {
        c.set_text("park this", window, cx);
        c.stash_draft(window, cx);
    });
    cx.executor().advance_clock(std::time::Duration::from_secs(1));
    cx.run_until_parked();
    assert_eq!(kept(cx), None, "a stashed draft is not kept as the draft");
    let _ = std::fs::remove_dir_all(&dir);
}

#[gpui_kit::test]
fn restoring_a_stash_entry_stashes_the_draft_in_the_box(cx: &mut TestAppContext) {
    let cx = open("empty", cx);
    let composer = composer_of(cx);
    composer.update_in(cx, |c, window, cx| {
        c.set_text("parked", window, cx);
        c.stash_draft(window, cx);
        c.set_text("in the box", window, cx);
        c.toggle_popup(Popup::Stash, cx);
        c.pick(Popup::Stash, "0".into(), window, cx);
    });
    composer.read_with(cx, |c, cx| {
        assert_eq!(c.text(cx), "parked");
        assert_eq!(c.stash().entries.iter().map(|e| e.prompt.as_str()).collect::<Vec<_>>(), ["in the box"]);
    });
}

#[gpui_kit::test]
fn a_queued_message_goes_once_a_turn_that_never_showed_running_has_ended(cx: &mut TestAppContext) {
    let cx = open("new-thread", cx);
    let composer = composer_of(cx);
    let started = starts(&composer, cx);
    composer.update_in(cx, |c, window, cx| {
        c.set_text("first", window, cx);
        c.submit(window, cx);
        c.set_text("second", window, cx);
        c.submit(window, cx);
        // The host took the first, and its turn had ended before the transcript was followed.
        c.sent(cx);
        c.set_turn(true, false, cx);
    });
    assert_eq!(*started.borrow(), ["first", "second"]);
}

#[gpui_kit::test]
fn fast_mode_follows_the_threads_last_turn_until_picked(cx: &mut TestAppContext) {
    let cx = open("empty", cx);
    let composer = composer_of(cx);
    composer.update(cx, |c, cx| {
        let mut ran = states::ran();
        ran.fast = Some(true);
        c.set_catalog(states::agents(), ran, Vec::new(), cx);
    });
    assert!(composer.read_with(cx, |c, _| c.fast()), "a thread that ran fast shows Fast on");
    composer.update_in(cx, |c, window, cx| c.pick(Popup::Reasoning, "fast:off".into(), window, cx));
    assert!(!composer.read_with(cx, |c, _| c.fast()));
}

#[gpui_kit::test]
fn a_stashed_item_chip_keeps_its_quoted_body(cx: &mut TestAppContext) {
    let cx = open("empty", cx);
    let composer = composer_of(cx);
    let item = HostItem { kind: "issue".into(), number: 7, title: "Jumps".into(), body: "It jumps.".into(), url: "https://x/7".into() };
    composer.update_in(cx, |c, window, cx| {
        c.editor().focus_handle(cx).focus(window, cx);
        c.set_text("#", window, cx);
        c.set_host_items(vec![item], None, cx);
    });
    cx.simulate_keystrokes("enter");
    composer.update_in(cx, |c, window, cx| c.stash_draft(window, cx));
    let stashed = composer.read_with(cx, |c, _| c.stash().entries[0].prompt.clone());
    assert!(stashed.contains("> It jumps."), "{stashed:?}");
}

#[gpui_kit::test]
fn a_refused_send_from_the_queue_goes_back_to_its_head_held(cx: &mut TestAppContext) {
    let cx = open("running", cx);
    let composer = composer_of(cx);
    let started = starts(&composer, cx);
    composer.update_in(cx, |c, window, cx| {
        c.set_text("queued one", window, cx);
        c.submit(window, cx);
        c.set_turn(true, false, cx);
    });
    let sent = started.borrow().clone();
    assert_eq!(sent, ["queued one"]);
    let out = desktop_gpui::screens::composer::Outgoing {
        prompt: "queued one".into(),
        files: Vec::new(),
        picks: Default::default(),
        fast: false,
        resume: None,
        from_queue: true,
    };
    composer.update_in(cx, |c, window, cx| c.refused_send(out, window, cx));
    composer.read_with(cx, |c, cx| {
        assert_eq!(c.queue().iter().map(|q| q.prompt.as_str()).collect::<Vec<_>>(), ["queued one"]);
        assert_eq!(c.text(cx), "", "the box keeps what the person was writing");
    });
    composer.update(cx, |c, cx| c.set_turn(true, false, cx));
    assert_eq!(started.borrow().len(), 1, "held until the person's next send");
}

#[gpui_kit::test]
fn enter_on_a_slash_draft_that_names_no_command_sends_nothing(cx: &mut TestAppContext) {
    let cx = open("empty", cx);
    let composer = composer_of(cx);
    let started = starts(&composer, cx);
    let review = Command { name: "review".into(), description: None, source: None };
    composer.update_in(cx, |c, window, cx| {
        c.set_catalog(states::agents(), states::ran(), vec![review], cx);
        c.editor().focus_handle(cx).focus(window, cx);
        c.set_text("/nope", window, cx);
    });
    assert!(composer.read_with(cx, |c, cx| c.send_held(cx)).is_some(), "the send button holds this draft");
    cx.simulate_keystrokes("enter");
    assert!(started.borrow().is_empty(), "Enter holds what the button holds");
    assert_eq!(composer.read_with(cx, |c, cx| c.text(cx)), "/nope");
}

#[gpui_kit::test]
fn a_second_composer_keeps_what_the_first_starred_and_stashed(cx: &mut TestAppContext) {
    let dir = std::env::temp_dir().join(format!("composer-shared-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    cx.update(|cx| local_store::init(dir.join("local-store.json"), cx));
    let cx = open("running", cx);
    let first = composer_of(cx);
    let second = cx.new_window_entity(desktop_gpui::screens::composer::Composer::new);
    first.update_in(cx, |c, window, cx| c.set_thread_key("thread-1".into(), window, cx));
    second.update_in(cx, |c, window, cx| {
        c.set_turn(true, true, cx);
        c.set_thread_key("thread-2".into(), window, cx);
    });
    first.update_in(cx, |c, window, cx| {
        c.toggle_favourite("claude:claude-opus-5-5".into(), cx);
        c.set_text("parked in the first", window, cx);
        c.stash_draft(window, cx);
    });
    second.update_in(cx, |c, window, cx| {
        c.set_text("queued in the second", window, cx);
        c.submit(window, cx);
        c.set_text("parked in the second", window, cx);
        c.stash_draft(window, cx);
    });
    let (favourites, stashed) = cx.update(|_, cx| {
        let s = cx.global::<local_store::LocalStore>();
        (store::favourites(s), store::stash(s).entries.into_iter().map(|e| e.prompt).collect::<Vec<_>>())
    });
    assert_eq!(favourites, ["claude:claude-opus-5-5"], "a send in the second keeps the first's star");
    assert_eq!(stashed, ["parked in the second", "parked in the first"], "and the first's stash entry");
    let _ = std::fs::remove_dir_all(&dir);
}

#[gpui_kit::test]
fn a_steer_the_turn_took_leaves_the_kept_queue(cx: &mut TestAppContext) {
    let dir = std::env::temp_dir().join(format!("composer-steer-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    cx.update(|cx| local_store::init(dir.join("local-store.json"), cx));
    let cx = open("running", cx);
    let composer = composer_of(cx);
    let kept = |cx: &mut gpui_kit::VisualTestContext| {
        cx.update(|_, cx| store::queue(cx.global::<local_store::LocalStore>(), "thread-1").rows.len())
    };
    composer.update_in(cx, |c, window, cx| {
        c.set_thread_key("thread-1".into(), window, cx);
        c.set_steer_preferred(true, cx);
        c.set_text("steer this", window, cx);
        c.submit(window, cx);
    });
    assert_eq!(kept(cx), 1, "a steer waits on the kept queue until the host answers");
    composer.update(cx, |c, cx| c.steered(true, cx));
    assert_eq!(kept(cx), 0, "a steer the turn took is not kept");
    let _ = std::fs::remove_dir_all(&dir);
}

#[gpui_kit::test]
fn a_refused_continue_keeps_the_conversation_for_the_next_send(cx: &mut TestAppContext) {
    use desktop_gpui::screens::composer::{ComposerEvent, Outgoing};
    let cx = open("resume-banner", cx);
    let composer = composer_of(cx);
    let seen = std::rc::Rc::new(std::cell::RefCell::new(Vec::<Outgoing>::new()));
    let sink = seen.clone();
    cx.update(|_, cx| {
        cx.subscribe(&composer, move |_, event: &ComposerEvent, _| {
            if let ComposerEvent::Start(out) = event {
                sink.borrow_mut().push(out.clone());
            }
        })
        .detach()
    });
    composer.update_in(cx, |c, window, cx| {
        c.set_text("go on", window, cx);
        c.submit(window, cx);
    });
    let first = seen.borrow()[0].clone();
    assert!(first.resume.is_some());
    composer.update_in(cx, |c, window, cx| c.refused_send(first, window, cx));
    composer.update_in(cx, |c, window, cx| c.submit(window, cx));
    let again = seen.borrow().get(1).cloned().expect("the retry goes");
    assert_eq!(again.prompt, "go on");
    assert_eq!(
        again.resume,
        Some(("5f0c9a52-7b1e-4d0a-9c3e-2a6b8d4e1f07".to_owned(), false)),
        "the retry still continues the conversation"
    );
}

#[gpui_kit::test]
fn opening_another_thread_leaves_the_first_threads_queue_where_it_was(cx: &mut TestAppContext) {
    let dir = std::env::temp_dir().join(format!("composer-switch-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    cx.update(|cx| local_store::init(dir.join("local-store.json"), cx));
    let cx = open("running", cx);
    let composer = composer_of(cx);
    composer.update_in(cx, |c, window, cx| {
        c.set_thread_key("thread-1".into(), window, cx);
        c.set_text("queued on the first", window, cx);
        c.submit(window, cx);
        c.set_thread_key("thread-2".into(), window, cx);
    });
    let kept = |key: &str, cx: &mut gpui_kit::VisualTestContext| {
        cx.update(|_, cx| store::queue(cx.global::<local_store::LocalStore>(), key).rows.len())
    };
    assert_eq!((kept("thread-1", cx), kept("thread-2", cx)), (1, 0), "the first thread's queue stays its own");
    let _ = std::fs::remove_dir_all(&dir);
}

#[gpui_kit::test]
fn a_second_message_while_a_steer_is_out_waits_its_turn(cx: &mut TestAppContext) {
    use desktop_gpui::screens::composer::ComposerEvent;
    let cx = open("running", cx);
    let composer = composer_of(cx);
    let started = starts(&composer, cx);
    let steered = std::rc::Rc::new(std::cell::RefCell::new(Vec::<String>::new()));
    let sink = steered.clone();
    cx.update(|_, cx| {
        cx.subscribe(&composer, move |_, event: &ComposerEvent, _| {
            if let ComposerEvent::Steer(out) = event {
                sink.borrow_mut().push(out.prompt.clone());
            }
        })
        .detach()
    });
    composer.update_in(cx, |c, window, cx| {
        c.set_steer_preferred(true, cx);
        c.set_text("one", window, cx);
        c.submit(window, cx);
        c.set_text("two", window, cx);
        c.submit(window, cx);
    });
    assert_eq!(*steered.borrow(), ["one"], "one steer is out at a time; the next message queues");
    composer.update(cx, |c, cx| {
        // The turn took "one", then ended.
        c.steered(true, cx);
        c.set_turn(true, false, cx);
    });
    assert_eq!(*started.borrow(), ["two"], "each message goes once");
}

gpui_kit::actions!(composer_keys, [Step]);

#[gpui_kit::test]
fn the_command_alt_arrows_step_the_threads_from_the_text_field(cx: &mut TestAppContext) {
    let stepped = std::rc::Rc::new(std::cell::Cell::new(0));
    let count = stepped.clone();
    cx.update(|cx| {
        gpui_kit::init(cx);
        // gpui-kit's own binding on macOS and Windows, which Linux leaves out, and the shell's thread step.
        let add_cursor = cx.build_action("input::AddCursorAbove", None).unwrap();
        cx.bind_keys([
            gpui_kit::KeyBinding::load(
                "secondary-alt-up",
                add_cursor,
                Some(std::rc::Rc::new(gpui_kit::KeyBindingContextPredicate::parse("Input").unwrap())),
                false,
                None,
                &gpui_kit::DummyKeyboardMapper,
            )
            .unwrap(),
            // The shell binds its keys in its own context, an ancestor of the text field's, as `Composer` stands in here.
            gpui_kit::KeyBinding::new("secondary-alt-up", Step, Some("Composer")),
        ]);
        cx.on_action(move |_: &Step, _| count.set(count.get() + 1));
    });
    let cx = open("empty", cx);
    let composer = composer_of(cx);
    composer.update_in(cx, |c, window, cx| c.editor().focus_handle(cx).focus(window, cx));
    cx.simulate_keystrokes("secondary-alt-up");
    assert_eq!(stepped.get(), 1, "the step wins over adding a cursor in the text field");
}

#[gpui_kit::test]
fn a_notice_shows_under_the_box_until_the_person_writes(cx: &mut TestAppContext) {
    let cx = open("running", cx);
    let composer = composer_of(cx);
    composer.update(cx, |c, cx| c.raise("Could not send now: wsp no longer knows this turn".into(), cx));
    cx.run_until_parked();
    assert!(cx.debug_bounds("composer-under").is_some(), "the notice is drawn under the box");
    composer.update_in(cx, |c, window, cx| c.editor().focus_handle(cx).focus(window, cx));
    cx.simulate_input("a");
    assert_eq!(composer.read_with(cx, |c, _| c.notice().map(str::to_owned)), None, "writing again puts it away");
}

/// The composer's column beside one more tab stop, as the window always has past the field.
struct WithStop {
    composer: gpui_kit::Entity<desktop_gpui::screens::composer::Composer>,
    stop: gpui_kit::FocusHandle,
}

impl gpui_kit::Render for WithStop {
    fn render(&mut self, _: &mut gpui_kit::Window, cx: &mut gpui_kit::Context<Self>) -> impl gpui_kit::IntoElement {
        use desktop_gpui::ui::ActivePalette as _;
        use gpui_kit::{InteractiveElement as _, ParentElement as _, Styled as _};
        let fresh = self.composer.read(cx).fresh();
        gpui_kit::div()
            .size_full()
            .child(desktop_gpui::screens::composer::screen::column(fresh, cx.palette().background, self.composer.clone()))
            .child(gpui_kit::div().track_focus(&self.stop).size(gpui_kit::px(10.)))
    }
}

#[gpui_kit::test]
fn tab_takes_the_menus_row_and_walks_the_focus_only_with_no_menu_open(cx: &mut TestAppContext) {
    use gpui_kit::AppContext as _;
    cx.update(|cx| {
        gpui_kit::init(cx);
        desktop_gpui::ui::init(desktop_gpui::ui::Side::Light, cx);
        desktop_gpui::screens::composer::init(cx);
    });
    let mut made = None;
    let (_, cx) = cx.add_window_view(|window, cx| {
        let composer = cx.new(|cx| desktop_gpui::screens::composer::Composer::new(window, cx));
        made = Some(composer.clone());
        let view = cx.new(|cx| WithStop { composer, stop: cx.focus_handle().tab_stop(true) });
        gpui_kit::component::Root::new(view, window, cx)
    });
    let composer = made.unwrap();
    composer.update_in(cx, |c, window, cx| {
        c.set_skills(vec![Skill { name: "tidy-imports".into(), description: Some("Sort the imports".into()) }], cx);
        c.editor().focus_handle(cx).focus(window, cx);
    });
    cx.run_until_parked();
    let in_field =
        |cx: &mut gpui_kit::VisualTestContext| cx.update(|window, cx| composer.read(cx).editor().focus_handle(cx).is_focused(window));
    cx.simulate_input("$ti");
    cx.simulate_keystrokes("tab");
    assert_eq!(composer.read_with(cx, |c, cx| c.text(cx)), "$tidy-imports ", "Tab takes the menu's row");
    assert!(in_field(cx), "and the field keeps the focus while the menu takes it");
    // With no menu open the web composer leaves Tab to the browser, which moves the focus on (ChatComposer.tsx); here
    // gpui-kit's Root walks it.
    cx.simulate_keystrokes("tab");
    assert_eq!(composer.read_with(cx, |c, cx| c.text(cx)), "$tidy-imports ", "the field writes no tab of its own");
    assert!(!in_field(cx), "Tab with no menu walks the focus on, as the browser does");
}

#[gpui_kit::test]
fn a_draft_typed_just_before_the_app_quits_is_kept(cx: &mut TestAppContext) {
    let dir = std::env::temp_dir().join(format!("composer-quit-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    cx.update(|cx| local_store::init(dir.join("local-store.json"), cx));
    let cx = open("empty", cx);
    let composer = composer_of(cx);
    composer.update_in(cx, |c, window, cx| {
        c.set_thread_key("thread-1".into(), window, cx);
        c.editor().focus_handle(cx).focus(window, cx);
    });
    cx.simulate_input("fix the scroll");
    drop(composer);
    cx.quit();
    let kept = store::draft(&local_store::LocalStore::open(dir.join("local-store.json")), "thread-1");
    assert_eq!(kept.map(|(t, _)| t).as_deref(), Some("fix the scroll"), "Cmd+Q inside the draft's 400 ms keeps it");
    let _ = std::fs::remove_dir_all(&dir);
}

#[gpui_kit::test]
fn enter_sends_a_slash_word_on_a_thread_that_lists_no_commands(cx: &mut TestAppContext) {
    let cx = open("new-thread", cx);
    let composer = composer_of(cx);
    let started = starts(&composer, cx);
    composer.update_in(cx, |c, window, cx| {
        c.editor().focus_handle(cx).focus(window, cx);
        c.set_text("/init", window, cx);
    });
    cx.simulate_keystrokes("enter");
    assert_eq!(*started.borrow(), ["/init"], "the web holds only a lone slash where the menu has nothing to check against");
}
