// SPDX-License-Identifier: AGPL-3.0-only
//! What the composer does with keys, menus and the store, against the web app's rules: Up and Down through the sent
//! prompts, the `#` and `$` menus, Cmd or Ctrl and a digit, the favourites, Fast mode, a thread pinned to its agent,
//! the drawer and the queue's bar, a long paste, a key typed outside any field, and what the store keeps.

#[path = "composer/harness.rs"]
mod harness;
#[path = "composer/states.rs"]
mod states;

use desktop_gpui::local_store;
use desktop_gpui::screens::composer::model::{Command, Drawer, HostItem, Limit, Skill, Slot, Tasks};
use desktop_gpui::screens::composer::screen::types_into_composer;
use desktop_gpui::screens::composer::{store, Bar, Popup};
use gpui_kit::{
    AppContext as _, BorrowAppContext as _, ClipboardItem, Focusable, KeyDownEvent, Keystroke, TestAppContext, VisualContext as _,
};
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
    composer.update_in(cx, |c, window, cx| {
        c.set_prompt(Some(states::asked()), window, cx);
        c.fold_prompt(window, cx);
        c.set_drawer(
            Drawer {
                limit: Some(Limit { title: "Usage limit reached".into(), line: "Claude Code resumes at 18:00".into(), ..Limit::default() }),
                slot: Some(Slot { line: "build-box is running 2 of 2 threads".into(), ..Slot::default() }),
                tasks: Some(Tasks { step: "Write the test".into(), done: 1, total: 3, steps: Vec::new() }),
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

/// Enter on a held send leaves the draft where it was typed and says why in a note toast, in the send button's hover.
#[gpui_kit::test]
fn enter_on_a_held_send_keeps_the_draft_and_says_why(cx: &mut TestAppContext) {
    use desktop_gpui::ui::toast::{self, Kind};
    let cx = open("empty", cx);
    let composer = composer_of(cx);
    let started = starts(&composer, cx);
    let review = Command { name: "review".into(), description: None, source: None };
    composer.update_in(cx, |c, window, cx| {
        c.set_catalog(states::agents(), states::ran(), vec![review], cx);
        c.editor().focus_handle(cx).focus(window, cx);
        c.set_text("/nope", window, cx);
    });
    let hover = composer.read_with(cx, |c, cx| c.send_held(cx)).expect("a slash word naming no command holds the send");
    cx.simulate_keystrokes("enter");
    cx.run_until_parked();
    assert_eq!(composer.read_with(cx, |c, cx| c.text(cx)), "/nope", "the draft stays");
    assert!(started.borrow().is_empty());
    let said: Vec<(Kind, String)> =
        cx.update(|window, cx| toast::of(window, cx).unwrap().read(cx).standing().iter().map(|n| (n.kind, n.text.to_string())).collect());
    assert_eq!(said, [(Kind::Note, hover)], "one note toast in the hover's words");
}

/// Send to thread while the agent's question stands in the box's place: the words join the draft and the question
/// keeps the keys, its digits still picking (sendToThread.ts focuses an editor that is not there while docked).
#[gpui_kit::test]
fn send_to_thread_under_a_standing_question_leaves_it_the_keys(cx: &mut TestAppContext) {
    use desktop_gpui::screens::pr_pane::comments::send_to_thread;
    let dir = tempfile::tempdir().unwrap();
    cx.update(|cx| local_store::init(dir.path().join("local-store.json"), cx));
    let cx = open("prompt-dock", cx);
    let composer = composer_of(cx);
    composer.update_in(cx, |c, window, cx| c.set_thread_key("thread-1".into(), window, cx));
    composer.update_in(cx, |c, window, cx| c.set_prompt(Some(states::asked()), window, cx));
    cx.run_until_parked();
    cx.update(|_, cx| send_to_thread("thread-1", "> a.rs:1", cx)).unwrap();
    cx.run_until_parked();
    assert_eq!(composer.read_with(cx, |c, cx| c.text(cx)), "> a.rs:1\n\n");
    cx.simulate_keystrokes("2");
    cx.run_until_parked();
    assert_eq!(composer.read_with(cx, |c, _| c.dock().map(|d| d.primary_word())), Some("Deny"), "the question still takes its digits");
}

/// Send to thread from the Changes or the Pull request pane, with the thread's composer on screen: the words land in
/// the open box at once under what it holds, words typed but not yet kept by the store included.
#[gpui_kit::test]
fn send_to_thread_from_a_pane_lands_in_the_open_composer(cx: &mut TestAppContext) {
    use desktop_gpui::screens::pr_pane::comments::send_to_thread;
    let dir = std::env::temp_dir().join(format!("composer-send-to-thread-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    cx.update(|cx| local_store::init(dir.join("local-store.json"), cx));
    let cx = open("empty", cx);
    let composer = composer_of(cx);
    composer.update_in(cx, |c, window, cx| {
        c.set_thread_key("thread-1".into(), window, cx);
        c.set_text("half a thought", window, cx);
    });
    cx.update(|_, cx| send_to_thread("thread-1", "> src/keep.ts:2\n\nWhy three?", cx)).unwrap();
    cx.run_until_parked();
    let sent = "half a thought\n\n> src/keep.ts:2\n\nWhy three?\n\n";
    composer.read_with(cx, |c, cx| {
        assert_eq!(c.text(cx), sent, "the open composer shows the words without a switch");
        assert_eq!(c.editor().read(cx).cursor(), sent.len(), "the caret after them");
    });
    assert!(cx.update(|window, cx| composer.read(cx).editor().focus_handle(cx).is_focused(window)), "the box has the keys");
    cx.executor().advance_clock(std::time::Duration::from_secs(1));
    cx.run_until_parked();
    let kept = cx.update(|_, cx| store::draft(cx.global::<local_store::LocalStore>(), "thread-1").map(|(t, _)| t));
    assert_eq!(kept.as_deref(), Some(sent), "and the store keeps them as the draft");

    // A thread with no composer open keeps the words for its next open.
    cx.update(|_, cx| send_to_thread("thread-2", "> b.rs:1", cx)).unwrap();
    cx.run_until_parked();
    let other = cx.update(|_, cx| store::draft(cx.global::<local_store::LocalStore>(), "thread-2").map(|(t, _)| t));
    assert_eq!(other.as_deref(), Some("> b.rs:1\n\n"));
    assert_eq!(composer.read_with(cx, |c, cx| c.text(cx)), sent, "the open box is left as it was");
    let _ = std::fs::remove_dir_all(&dir);
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
        fork: None,
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
fn each_thread_keeps_its_own_draft_across_a_switch(cx: &mut TestAppContext) {
    let dir = std::env::temp_dir().join(format!("composer-switch-draft-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    cx.update(|cx| local_store::init(dir.join("local-store.json"), cx));
    let cx = open("empty", cx);
    let composer = composer_of(cx);
    let text = |cx: &mut gpui_kit::VisualTestContext| composer.read_with(cx, |c, cx| c.text(cx));
    let kept = |cx: &mut gpui_kit::VisualTestContext, key: &str| {
        cx.update(|_, cx| store::draft(cx.global::<local_store::LocalStore>(), key).map(|(t, _)| t))
    };
    composer.update_in(cx, |c, window, cx| {
        c.set_thread_key("short-1".into(), window, cx);
        c.editor().focus_handle(cx).focus(window, cx);
    });
    cx.simulate_input("x");
    composer.update_in(cx, |c, window, cx| c.set_thread_key("short-2".into(), window, cx));
    assert_eq!(text(cx), "", "a thread with no draft opens on an empty box");
    cx.executor().advance_clock(std::time::Duration::from_secs(1));
    cx.run_until_parked();
    assert_eq!(kept(cx, "short-2"), None, "the first thread's words are not kept under the second's key");
    composer.update_in(cx, |c, window, cx| c.set_thread_key("short-1".into(), window, cx));
    assert_eq!(text(cx), "x", "the first thread's draft is back");
    let _ = std::fs::remove_dir_all(&dir);
}

#[gpui_kit::test]
fn a_message_queued_before_a_switch_still_starts_the_next_turn_with_its_files(cx: &mut TestAppContext) {
    use desktop_gpui::screens::composer::model::File;
    use desktop_gpui::screens::composer::ComposerEvent;
    let dir = std::env::temp_dir().join(format!("composer-switch-queue-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    cx.update(|cx| local_store::init(dir.join("local-store.json"), cx));
    let cx = open("running", cx);
    let composer = composer_of(cx);
    let started = std::rc::Rc::new(std::cell::RefCell::new(Vec::new()));
    let sink = started.clone();
    cx.update(|_, cx| {
        cx.subscribe(&composer, move |_, e: &ComposerEvent, _| {
            if let ComposerEvent::Start(out) = e {
                sink.borrow_mut().push((out.prompt.clone(), out.files.len()));
            }
        })
        .detach()
    });
    let file = File { name: "notes.txt".into(), media_type: "text/plain".into(), bytes: std::sync::Arc::new(b"hi".to_vec()) };
    composer.update_in(cx, |c, window, cx| {
        c.set_thread_key("short-1".into(), window, cx);
        c.attach(vec![file], cx);
        c.set_text("then the sidebar", window, cx);
        c.submit(window, cx);
        c.set_thread_key("short-2".into(), window, cx);
        c.set_thread_key("short-1".into(), window, cx);
        c.set_thread_key("short-1".into(), window, cx);
        c.set_turn(true, false, cx);
    });
    assert_eq!(
        *started.borrow(),
        [("then the sidebar".to_owned(), 1)],
        "a glance at another thread neither holds the queue as a launch does nor drops its files"
    );
    let _ = std::fs::remove_dir_all(&dir);
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
        use gpui_kit::{InteractiveElement as _, ParentElement as _, Styled as _};
        let fresh = self.composer.read(cx).fresh();
        gpui_kit::div()
            .size_full()
            .child(desktop_gpui::screens::composer::screen::column(fresh, self.composer.clone()))
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
    let vcx = open("empty", cx);
    let composer = composer_of(vcx);
    composer.update_in(vcx, |c, window, cx| {
        c.set_thread_key("thread-1".into(), window, cx);
        c.editor().focus_handle(cx).focus(window, cx);
    });
    vcx.simulate_input("fix the scroll");
    // The app's own context quits: a quit through the window's context frees that context before it reaches the app.
    // The composer is held through it, so its release save cannot stand in for its quit save.
    cx.quit();
    let kept = store::draft(&local_store::LocalStore::open(dir.join("local-store.json")), "thread-1");
    assert_eq!(kept.map(|(t, _)| t).as_deref(), Some("fix the scroll"), "Cmd+Q inside the draft's 300 ms keeps it");
    drop(composer);
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

fn out(prompt: &str, from_queue: bool) -> desktop_gpui::screens::composer::Outgoing {
    desktop_gpui::screens::composer::Outgoing {
        prompt: prompt.into(),
        files: Vec::new(),
        picks: Default::default(),
        fast: false,
        resume: None,
        from_queue,
        fork: None,
    }
}

#[gpui_kit::test]
fn a_steer_answered_after_a_switch_leaves_its_own_threads_queue(cx: &mut TestAppContext) {
    let dir = std::env::temp_dir().join(format!("composer-steer-switch-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    cx.update(|cx| local_store::init(dir.join("local-store.json"), cx));
    let cx = open("running", cx);
    let composer = composer_of(cx);
    let started = starts(&composer, cx);
    composer.update_in(cx, |c, window, cx| {
        c.set_thread_key("thread-1".into(), window, cx);
        c.set_steer_preferred(true, cx);
        c.set_text("turn left", window, cx);
        c.submit(window, cx);
        c.set_thread_key("thread-2".into(), window, cx);
    });
    // The host takes the steer while the box shows the second thread.
    composer.update(cx, |c, cx| c.steered_on("thread-1", true, cx));
    composer.update_in(cx, |c, window, cx| c.set_thread_key("thread-1".into(), window, cx));
    assert_eq!(composer.read_with(cx, |c, _| c.queue().len()), 0, "the taken steer left its own thread's queue");
    composer.update(cx, |c, cx| c.set_turn(true, false, cx));
    assert!(started.borrow().is_empty(), "the steer is not sent again as the next turn");
    let _ = std::fs::remove_dir_all(&dir);
}

#[gpui_kit::test]
fn a_switch_lets_go_of_what_the_last_thread_held(cx: &mut TestAppContext) {
    use desktop_gpui::screens::composer::model::Picks;
    use desktop_gpui::screens::composer::{Banner, ComposerEvent};
    let cx = open("attachments", cx);
    let composer = composer_of(cx);
    let asked = std::rc::Rc::new(std::cell::Cell::new(0));
    let sink = asked.clone();
    cx.update(|_, cx| {
        cx.subscribe(&composer, move |_, e: &ComposerEvent, _| {
            if matches!(e, ComposerEvent::Files) {
                sink.set(sink.get() + 1);
            }
        })
        .detach()
    });
    composer.update_in(cx, |c, window, cx| {
        c.set_thread_key("thread-1".into(), window, cx);
        c.set_picks(Picks { model: Some("claude-sonnet-5".into()), ..Picks::default() }, cx);
        c.set_banner(Some(Banner::resume("conv".into(), "Old chat", false, "Claude Code")), cx);
        c.set_prompt(Some(states::asked()), window, cx);
        c.raise("Could not send now".into(), cx);
        c.set_text("@", window, cx);
        c.set_folder_files(vec!["a.rs".into()], cx);
    });
    assert_eq!(asked.get(), 1);
    composer.update_in(cx, |c, window, cx| c.set_thread_key("thread-2".into(), window, cx));
    composer.read_with(cx, |c, _| {
        assert_eq!(c.picks(), &Picks::default(), "the model picked on the first thread");
        assert!(c.files().is_empty(), "its files");
        assert!(c.banner().is_none(), "its banner");
        assert!(c.dock().is_none(), "its prompt");
        assert!(c.notice().is_none(), "its notice");
    });
    composer.update_in(cx, |c, window, cx| c.set_text("@", window, cx));
    assert_eq!(asked.get(), 2, "the second thread's folder is listed for itself");
}

#[gpui_kit::test]
fn a_refusal_from_the_last_threads_link_lands_in_that_thread(cx: &mut TestAppContext) {
    let dir = std::env::temp_dir().join(format!("composer-late-refusal-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    cx.update(|cx| local_store::init(dir.join("local-store.json"), cx));
    let cx = open("empty", cx);
    let composer = composer_of(cx);
    composer.update_in(cx, |c, window, cx| {
        c.set_thread_key("thread-1".into(), window, cx);
        c.set_thread_key("thread-2".into(), window, cx);
        c.set_text("the second's words", window, cx);
        c.refused_on("thread-1", out("the first's send", false), window, cx);
        c.refused_on("thread-1", out("the first's queued send", true), window, cx);
    });
    assert_eq!(composer.read_with(cx, |c, cx| c.text(cx)), "the second's words", "nothing lands in the second thread's box");
    assert!(composer.read_with(cx, |c, _| c.queue().is_empty()), "or its queue");
    composer.update_in(cx, |c, window, cx| c.set_thread_key("thread-1".into(), window, cx));
    composer.read_with(cx, |c, cx| {
        assert_eq!(c.text(cx), "the first's send", "the refused words wait in the first thread's box");
        assert_eq!(c.queue().iter().map(|q| q.prompt.as_str()).collect::<Vec<_>>(), ["the first's queued send"]);
    });
    let _ = std::fs::remove_dir_all(&dir);
}

#[gpui_kit::test]
fn the_draft_is_written_300_ms_after_typing_and_at_once_on_send_stash_and_switch(cx: &mut TestAppContext) {
    let dir = std::env::temp_dir().join(format!("composer-draft-at-once-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    cx.update(|cx| local_store::init(dir.join("local-store.json"), cx));
    let cx = open("empty", cx);
    let composer = composer_of(cx);
    let kept = |cx: &mut gpui_kit::VisualTestContext, key: &str| {
        cx.update(|_, cx| store::draft(cx.global::<local_store::LocalStore>(), key).map(|(t, _)| t))
    };
    composer.update_in(cx, |c, window, cx| {
        c.set_thread_key("thread-1".into(), window, cx);
        c.set_text("fix it", window, cx);
    });
    cx.executor().advance_clock(std::time::Duration::from_millis(250));
    assert_eq!(kept(cx, "thread-1"), None, "nothing is written while the person types");
    cx.executor().advance_clock(std::time::Duration::from_millis(60));
    assert_eq!(kept(cx, "thread-1").as_deref(), Some("fix it"), "written 300 ms after the last edit");
    composer.update_in(cx, |c, window, cx| c.submit(window, cx));
    assert_eq!(kept(cx, "thread-1"), None, "a send empties the kept draft at once");
    composer.update_in(cx, |c, window, cx| {
        c.set_text("park this", window, cx);
        c.stash_draft(window, cx);
    });
    assert_eq!(kept(cx, "thread-1"), None, "a stash empties it at once");
    composer.update_in(cx, |c, window, cx| {
        c.set_text("half typed", window, cx);
        c.set_thread_key("thread-2".into(), window, cx);
    });
    assert_eq!(kept(cx, "thread-1").as_deref(), Some("half typed"), "a switch keeps the leaving draft at once");
    let _ = std::fs::remove_dir_all(&dir);
}

#[gpui_kit::test]
fn denys_field_stands_in_its_rows_right_column_wide_enough_for_a_sentence(cx: &mut TestAppContext) {
    let cx = open("prompt-dock", cx);
    let composer = composer_of(cx);
    assert!(cx.debug_bounds("control").is_none(), "Allow opens no field");
    cx.simulate_keystrokes("2");
    cx.run_until_parked();
    assert_eq!(composer.read_with(cx, |c, _| c.dock().map(|d| d.primary_word())), Some("Deny"));
    let (field, dock, copy) =
        (cx.debug_bounds("control").unwrap(), cx.debug_bounds("prompt-dock").unwrap(), cx.debug_bounds("copy-row").unwrap());
    assert_eq!(field.size, gpui_kit::size(gpui_kit::px(384.), gpui_kit::px(30.)), "`ROW_FIELD` at `w-96`");
    // The body's 20 px side, the card's 1 px edge and the row's 20 px inset.
    assert_eq!(dock.right() - field.right(), gpui_kit::px(41.), "at the row's right edge");
    // Under the copy row: the body's 20 px gap, the card's edge, the Allow row's 60 px floor, then the middle of what
    // Deny's floor holds under its 1 px rule.
    assert_eq!(field.center().y - copy.bottom(), gpui_kit::px(20. + 1. + 60. + 1. + 29.5), "in the Deny row, so no row moves");
}

#[gpui_kit::test]
fn a_send_out_when_the_person_switched_away_and_back_keeps_the_next_message_waiting(cx: &mut TestAppContext) {
    let cx = open("running", cx);
    let composer = composer_of(cx);
    let started = starts(&composer, cx);
    composer.update_in(cx, |c, window, cx| {
        c.set_thread_key("thread-1".into(), window, cx);
        c.set_text("first queued", window, cx);
        c.submit(window, cx);
        c.set_text("second queued", window, cx);
        c.submit(window, cx);
        // The turn ends: the queue's head goes, and the host has not answered it yet.
        c.set_turn(true, false, cx);
        c.set_thread_key("thread-2".into(), window, cx);
        c.set_thread_key("thread-1".into(), window, cx);
        // The link stands again and the transcript still says no turn runs.
        c.set_hold(None, cx);
        c.set_turn(true, false, cx);
    });
    assert_eq!(*started.borrow(), ["first queued"], "the second waits for the host to answer the first");
    composer.update(cx, |c, cx| {
        c.sent_on("thread-1", cx);
        c.set_turn(true, true, cx);
        c.set_turn(true, false, cx);
    });
    assert_eq!(*started.borrow(), ["first queued", "second queued"], "and goes once the first turn ends");
}

#[gpui_kit::test]
fn a_forks_new_branch_is_offered_from_the_sidebars_listing(cx: &mut TestAppContext) {
    use desktop_gpui::screens::composer::fork::branch_from;
    use desktop_gpui::sidebar::host::Change;
    use desktop_gpui::sidebar::Sidebar;
    use serde_json::json;
    let cx = open("empty", cx);
    let sidebar = cx.update(|_, cx| cx.new(Sidebar::new));
    let feed = |kind: &str, git: bool, daemon: u64, cx: &mut gpui_kit::VisualTestContext| {
        sidebar.update(cx, |s, cx| {
            s.apply(
                Change::Workspaces(vec![json!({ "id": "w1", "name": "w1", "kind": kind, "folder": "/repo", "project": { "id": "p1" } })]),
                cx,
            );
            let project = if git {
                json!({ "id": "p1", "name": "repo", "git": { "remote": "origin" } })
            } else {
                json!({ "id": "p1", "name": "repo" })
            };
            s.apply(Change::Projects(vec![project]), cx);
            s.apply(Change::Places(vec![json!({ "id": "here", "name": "studio", "kind": "computer", "daemonVersion": daemon })]), cx);
        });
        sidebar.read_with(cx, |s, _| branch_from(s, "w1", None))
    };
    assert_eq!(feed("local", true, 150, cx).as_deref(), Some("/repo"), "a git copy on a daemon that cuts worktrees offers a branch");
    assert_eq!(feed("local", true, 149, cx), None, "a daemon too old to cut at a checkpoint offers none");
    assert_eq!(feed("local", false, 150, cx), None, "a folder that is no git checkout offers none");
    assert_eq!(feed("place", true, 150, cx), None, "a thread on another computer offers none");
}

#[gpui_kit::test]
fn a_send_refused_after_a_switch_comes_back_with_its_files(cx: &mut TestAppContext) {
    use desktop_gpui::screens::composer::model::File;
    use desktop_gpui::screens::composer::ComposerEvent;
    let dir = std::env::temp_dir().join(format!("composer-refused-files-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    cx.update(|cx| local_store::init(dir.join("local-store.json"), cx));
    let cx = open("empty", cx);
    let composer = composer_of(cx);
    let out = std::rc::Rc::new(std::cell::RefCell::new(None));
    let sink = out.clone();
    cx.update(|_, cx| {
        cx.subscribe(&composer, move |_, e: &ComposerEvent, _| {
            if let ComposerEvent::Start(o) = e {
                *sink.borrow_mut() = Some(o.clone());
            }
        })
        .detach()
    });
    let file = File { name: "notes.txt".into(), media_type: "text/plain".into(), bytes: std::sync::Arc::new(b"hi".to_vec()) };
    composer.update_in(cx, |c, window, cx| {
        c.set_thread_key("short-1".into(), window, cx);
        c.attach(vec![file], cx);
        c.set_text("look at these notes", window, cx);
        c.submit(window, cx);
        c.set_thread_key("short-2".into(), window, cx);
    });
    let sent = out.borrow_mut().take().expect("the send");
    composer.update_in(cx, |c, window, cx| {
        c.refused_on("short-1", sent, window, cx);
        c.set_thread_key("short-1".into(), window, cx);
    });
    let (text, files) = composer.read_with(cx, |c, cx| (c.text(cx), c.files().iter().map(|f| f.name.clone()).collect::<Vec<_>>()));
    assert_eq!(text, "look at these notes");
    assert_eq!(files, ["notes.txt"], "the refused send's files were dropped");
    let _ = std::fs::remove_dir_all(&dir);
}

#[gpui_kit::test]
fn showing_a_thread_again_leaves_its_kept_draft_as_it_was(cx: &mut TestAppContext) {
    let dir = std::env::temp_dir().join(format!("composer-reshow-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    cx.update(|cx| local_store::init(dir.join("local-store.json"), cx));
    cx.update(|cx| {
        cx.update_global(|s: &mut local_store::LocalStore, _| store::set_draft_at(s, "short-1", "fix the scroll", 3, 1_000)).unwrap()
    });
    let cx = open("empty", cx);
    let composer = composer_of(cx);
    let edited = |cx: &mut gpui_kit::VisualTestContext| {
        cx.update(|_, cx| {
            let kept = cx.global::<local_store::LocalStore>().get(store::DRAFTS_KEY).cloned().unwrap_or_default();
            (
                kept.pointer("/state/drafts/short-1/editedAt").and_then(|v| v.as_u64()),
                kept.pointer("/state/drafts/short-1/cursor").and_then(|v| v.as_u64()),
            )
        })
    };
    composer.update_in(cx, |c, window, cx| c.set_thread_key("short-1".into(), window, cx));
    assert_eq!(composer.read_with(cx, |c, cx| c.editor().read(cx).cursor()), 3, "the caret stands where the store kept it");
    composer.update_in(cx, |c, window, cx| {
        c.set_thread_key("short-2".into(), window, cx);
        c.set_thread_key("short-1".into(), window, cx);
    });
    cx.executor().advance_clock(std::time::Duration::from_secs(1));
    cx.run_until_parked();
    assert_eq!(edited(cx), (Some(1_000), Some(3)), "showing the thread again wrote its draft anew");
    let _ = std::fs::remove_dir_all(&dir);
}

#[gpui_kit::test]
fn a_switch_off_a_running_thread_draws_no_stop_for_the_next(cx: &mut TestAppContext) {
    let cx = open("running", cx);
    let composer = composer_of(cx);
    composer.update_in(cx, |c, window, cx| c.set_thread_key("short-1".into(), window, cx));
    cx.update(|window, _| window.refresh());
    cx.run_until_parked();
    assert!(cx.debug_bounds("composer-stop").is_some(), "the running thread offers Stop");
    composer.update_in(cx, |c, window, cx| c.set_thread_key("short-2".into(), window, cx));
    cx.update(|window, _| window.refresh());
    cx.run_until_parked();
    assert!(!composer.read_with(cx, |c, _| c.running()), "the next thread's box still read as running");
    assert!(cx.debug_bounds("composer-stop").is_none(), "Stop stood over the next thread's box");
}

#[test]
fn a_forks_first_send_names_its_source_turn_and_the_branch_picked() {
    use desktop_gpui::screens::composer::link::fork_params;
    use desktop_gpui::screens::composer::{Fork, Outgoing};
    let out = Outgoing {
        prompt: "try it on a branch".into(),
        files: Vec::new(),
        picks: Default::default(),
        fast: false,
        resume: None,
        from_queue: false,
        fork: None,
    };
    let fork = |branch: Option<&str>| Fork {
        workspace: "w".into(),
        thread: "t".into(),
        turn: Some("u1".into()),
        branch: branch.map(str::to_owned),
    };
    let on_branch = fork_params(&fork(Some("main-fork")), &out);
    assert_eq!(on_branch["fork"], serde_json::json!({ "threadId": "t", "turnId": "u1" }));
    assert_eq!(on_branch["branch"], "main-fork", "New branch goes with the send");
    assert!(fork_params(&fork(None), &out).get("branch").is_none(), "This folder names no branch");
}
