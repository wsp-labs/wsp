// SPDX-License-Identifier: AGPL-3.0-only

use super::*;

#[gpui_kit::test]
fn a_turns_changed_files_draw_the_panes_tree_shut_and_expand_all(cx: &mut TestAppContext) {
    let (_, cx) = window(changes(), cx);
    let card = bounds(cx, "changed-files");
    let head = bounds(cx, "changed-head");
    // `px-3 py-2` round the 24 px acts; the tree `p-2`, each row the panes' row and 2 px apart, every folder shut.
    assert_eq!(head.size.height, px(40.));
    assert!(cx.debug_bounds("changed-files-tree").is_some(), "the card draws the panes' tree");
    let row = crate::screens::pr_pane::changes::tree::ROW;
    assert_eq!(card.size.height, head.size.height + px(8.) + row + px(2.) + row + px(8.));
    let folder = bounds(cx, "tree-row:src/ui");
    assert_eq!(folder.left() - card.left(), px(8.));
    assert!(cx.debug_bounds("tree-row:src/ui/a.rs").is_none(), "folders start shut");
    cx.simulate_click(folder.center(), Modifiers::none());
    cx.run_until_parked();
    assert_eq!(bounds(cx, "changed-files").size.height, card.size.height + (px(2.) + row) * 2.);
    cx.simulate_click(folder.center(), Modifiers::none());
    cx.run_until_parked();
    assert_eq!(bounds(cx, "changed-files").size.height, card.size.height);
    let all = bounds(cx, "expand-all");
    cx.simulate_click(all.center(), Modifiers::none());
    cx.run_until_parked();
    assert_eq!(bounds(cx, "changed-files").size.height, card.size.height + (px(2.) + row) * 2.);
    assert!(cx.debug_bounds("tree-row:src/ui/c.rs").is_some(), "Expand all folders opens every folder");
    let open_diff = bounds(cx, "open-diff");
    assert_eq!((open_diff.size.height, head.right() - open_diff.right()), (px(24.), px(12.)));
    assert!(cx.debug_bounds("open-diff-word").is_some(), "a card 384 px wide or more says Open diff");
}

#[gpui_kit::test]
fn open_diffs_word_goes_under_384_px_and_its_tooltip_says_the_full_diff(cx: &mut TestAppContext) {
    let (_, cx) = window(changes(), cx);
    cx.simulate_resize(size(px(420.), px(1000.)));
    cx.run_until_parked();
    // The card's new width is read in the frame the resize draws; the next frame, which it asks for, draws by it.
    cx.update(|window, cx| window.simulate_next_frame(cx));
    cx.run_until_parked();
    assert!(bounds(cx, "changed-files").size.width < px(384.));
    assert!(cx.debug_bounds("open-diff-word").is_none(), "Open diff kept its word under 384 px");
    assert_eq!(bounds(cx, "open-diff").size.width, px(30.), "the xs button keeps its padding round the glyph");
    let at = bounds(cx, "open-diff").center();
    cx.simulate_mouse_move(at, None, Modifiers::none());
    cx.executor().advance_clock(std::time::Duration::from_secs(1));
    cx.run_until_parked();
    assert!(cx.debug_bounds("tooltip").is_some(), "no Open the full diff tooltip");
}

#[gpui_kit::test]
fn the_folders_others_stand_under_their_own_words_and_tree(cx: &mut TestAppContext) {
    let mut events = changes();
    let others = json!([{"path": "docs/NOTES.md", "kind": "modified", "additions": 1, "deletions": 1}]);
    let files = json!([
        {"path": "b.rs", "kind": "added", "additions": 9, "deletions": 0},
        {"path": "docs/a.md", "kind": "added", "additions": 2, "deletions": 0}
    ]);
    events[4] = base("session.changes", 5, json!({"from": "a", "to": "b", "moved": [], "others": others, "files": files}));
    let (_, cx) = window(events, cx);
    let also = bounds(cx, "changed-also");
    let own = bounds(cx, "tree-row:b.rs");
    // Both trees hold a docs folder; the map keeps the last drawn, the others'.
    let theirs = bounds(cx, "tree-row:docs");
    assert!(own.bottom() < also.top() && also.bottom() < theirs.top(), "the others' words stand between the two trees");
    cx.simulate_click(theirs.center(), Modifiers::none());
    cx.run_until_parked();
    assert!(cx.debug_bounds("tree-row:docs/NOTES.md").is_some(), "a press on the others' docs left it shut");
    assert!(cx.debug_bounds("tree-row:docs/a.md").is_none(), "a press on the others' docs opened the turn's own");
    // The turn's own docs is its tree's first row, folders before files: 8 px under the head, 24 px high.
    let head = bounds(cx, "changed-head");
    cx.simulate_click(gpui_kit::point(own.center().x, head.bottom() + px(20.)), Modifiers::none());
    cx.run_until_parked();
    assert!(cx.debug_bounds("tree-row:docs/a.md").is_some(), "a press on the turn's own docs left it shut");
    assert!(cx.debug_bounds("tree-row:docs/NOTES.md").is_some(), "a press on the turn's own docs shut the others'");
}

#[gpui_kit::test]
fn the_cards_head_sticks_8_px_under_the_lists_top_while_its_card_is_in_view(cx: &mut TestAppContext) {
    let (view, cx) = window(changes(), cx);
    cx.simulate_resize(size(px(900.), px(300.)));
    cx.run_until_parked();
    let all = bounds(cx, "expand-all").center();
    cx.simulate_click(all, Modifiers::none());
    cx.run_until_parked();
    let card = bounds(cx, "changed-files");
    let list = view.update(cx, |v, _| v.list().viewport_bounds());
    let past = card.top() - list.top() + px(30.);
    view.update(cx, |v, cx| {
        v.list().scroll_by(past);
        cx.notify();
    });
    cx.run_until_parked();
    let (card, head) = (bounds(cx, "changed-files"), bounds(cx, "changed-head"));
    assert!(card.top() < list.top(), "the card's top went past the list's");
    assert_eq!(head.top(), list.top() + px(8.), "the head did not stick 8 px under the list's top");
}
