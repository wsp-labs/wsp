// SPDX-License-Identifier: AGPL-3.0-only

use super::*;

#[gpui_kit::test]
fn a_long_plan_folds_behind_expand_plan_and_its_menu_copies_it(cx: &mut TestAppContext) {
    let long: String = std::iter::once("# Fix the build\n\n".to_owned()).chain((0..30).map(|n| format!("- step {n}\n"))).collect();
    let (view, cx) = window(plan(&long), cx);
    let card = bounds(cx, "plan");
    let title = bounds(cx, "plan-title");
    // `border p-5`; the title's 20 px line; the menu's `icon-xs` keycap at the right.
    // The head is as tall as the button, the title centred in it.
    assert_eq!((title.left() - card.left(), title.top() - card.top(), title.size.height), (px(21.), px(23.), px(20.)));
    let menu = bounds(cx, "menu-button");
    assert_eq!((menu.size, card.right() - menu.right()), (size(px(24.), px(24.)), px(21.)));
    // Folded: `mt-4`, the preview at most `max-h-104`, `mt-4`, the 32 px Expand plan, the card's padding and border.
    let folded = card.size.height;
    let body = bounds(cx, "plan-body");
    assert!(body.size.height <= px(416.), "{body:?}");
    assert_eq!(folded, px(1. + 20. + 24. + 16.) + body.size.height + px(16. + 32. + 20. + 1.));
    let expand = card.bottom() - px(21. + 16.);
    cx.simulate_click(gpui_kit::point(card.center().x, expand), Modifiers::none());
    cx.run_until_parked();
    assert!(view.read_with(cx, |v, _| v.rows().iter().any(|r| matches!(r.kind, RowKind::Plan { open: true, .. }))));
    assert!(bounds(cx, "plan").size.height > folded + px(100.), "the plan opens past its fold");
    cx.simulate_click(menu.center(), Modifiers::none());
    cx.run_until_parked();
    let popup = bounds(cx, "plan-menu");
    let copy = bounds(cx, "plan-copy");
    // `min-w-32 p-1`, its rows `min-h-7`, its right edge under the button's.
    assert!(popup.size.width >= px(128.));
    assert_eq!(
        (copy.size.height, copy.top() - popup.top(), popup.right(), popup.top() - menu.bottom()),
        (px(28.), px(5.), menu.right(), px(4.))
    );
    cx.simulate_click(copy.center(), Modifiers::none());
    cx.run_until_parked();
    assert_eq!(cx.read_from_clipboard().and_then(|c| c.text()), Some(format!("{}\n", long.trim_end())));
    assert!(cx.debug_bounds("plan-menu").is_none(), "a pick shuts the menu");
}

#[gpui_kit::test]
fn a_path_in_a_plan_draws_as_a_file_chip(cx: &mut TestAppContext) {
    let mut events = plan("# Fix the build\n\n- Change `src/main.rs` so it holds.\n");
    events[0] = base("session.start", 1, json!({"prompt": "plan it", "cwd": "/w"}));
    let (_, cx) = window(events, cx);
    assert!(cx.debug_bounds("file-chip").is_some(), "the plan drew src/main.rs as code, not as today's file chip");
}

#[gpui_kit::test]
fn a_plan_downloads_where_the_save_dialog_says(cx: &mut TestAppContext) {
    assert_eq!(plan_filename(Some("Fix the build's (CI) step!")), "fix-the-builds-ci-step.md");
    assert_eq!(plan_filename(None), "plan.md");
    let text = "# Fix the build\n\n- step one\n";
    let (_, cx) = window(plan(text), cx);
    let at = bounds(cx, "menu-button").center();
    cx.simulate_click(at, Modifiers::none());
    cx.run_until_parked();
    let at = bounds(cx, "plan-download").center();
    cx.simulate_click(at, Modifiers::none());
    cx.run_until_parked();
    assert!(cx.did_prompt_for_new_path(), "Download as markdown wrote without asking where");
    let folder = tempfile::tempdir().unwrap();
    let chosen = folder.path().join("mine.md");
    let to = chosen.clone();
    cx.simulate_new_path_selection(move |_| Some(to));
    cx.run_until_parked();
    assert_eq!(std::fs::read_to_string(&chosen).unwrap(), format!("{}\n", text.trim_end()));
}
